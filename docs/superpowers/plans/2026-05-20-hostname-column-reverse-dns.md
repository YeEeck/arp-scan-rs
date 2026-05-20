# Hostname Column Reverse DNS Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `Hostname` column to the live scan results and fill it asynchronously via reverse DNS without changing ARP scan success semantics.

**Architecture:** Keep ARP discovery as the source of truth for scan progress and completion. Add a separate hostname-resolution path that runs after each host is discovered, emits optional hostname update events, and is ignored when stale. Thread hostname through the Rust UI state first, then expose it in the Slint model and panel once the data plumbing is stable.

**Tech Stack:** Rust 2024, Slint 1.14, `dns-lookup` 3.0.1, std `mpsc`, `Arc<AtomicBool>`, Windows `SendARP`

---

## File Structure

- Modify: `Cargo.toml`
  - Add the reverse DNS lookup crate.
- Modify: `Cargo.lock`
  - Resolve the new dependency.
- Modify: `src/scan_master.rs`
  - Re-export the hostname resolver types and the new runtime helper.
- Create: `src/scan_master/hostname.rs`
  - Own the reverse DNS resolver trait and the system implementation.
- Modify: `src/scan_master/runtime.rs`
  - Emit hostname update events and spawn lookup work after host discovery.
- Modify: `src/ui_state.rs`
  - First accept hostname events as a no-op compatibility bridge, then store hostname on rows and update them in place.
- Modify: `src/main.rs`
  - Eventually map the hostname field into the Slint row model.
- Modify: `ui/main.slint`
  - Eventually add the `hostname` field to `ResultListData` and render it in the result panel.
- Modify: `tests/scan_runtime.rs`
  - Verify hostname lookup is asynchronous and does not block scan completion.
- Modify: `tests/ui_state.rs`
  - Verify hostname updates land on the correct row and stale hostname updates are ignored.
- Modify: `tests/ui_smoke.rs`
  - Verify the Slint window accepts rows that include hostnames once the data model is extended.
- Modify: `tests/result_list_panel_smoke.rs`
  - Verify the result panel accepts hostname-bearing rows once the model and panel are extended.

### Task 1: Add Reverse DNS Events To The Scan Runtime

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `src/scan_master.rs`
- Create: `src/scan_master/hostname.rs`
- Modify: `src/scan_master/runtime.rs`
- Modify: `src/ui_state.rs`
- Modify: `tests/scan_runtime.rs`

- [ ] **Step 1: Write the failing test**

Add this test to `tests/scan_runtime.rs`:

```rust
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use arp_scan_rs::scan_master::{
    start_scan_with_probe_and_hostname_resolver, ArpProbe, HostnameResolver, ScanEvent, ScanTask,
};

#[derive(Clone)]
struct QuickProbe;

impl ArpProbe for QuickProbe {
    fn probe(&self, ip: &str) -> io::Result<Option<[u8; 6]>> {
        if ip.ends_with(".1") {
            Ok(Some([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x01]))
        } else {
            Ok(None)
        }
    }
}

#[derive(Clone)]
struct BlockingHostnameResolver {
    started_tx: mpsc::Sender<String>,
    permits: Arc<Mutex<mpsc::Receiver<()>>>,
}

impl HostnameResolver for BlockingHostnameResolver {
    fn resolve(&self, ip: &str) -> Option<String> {
        self.started_tx
            .send(ip.to_string())
            .expect("test should observe hostname resolution start");

        self.permits
            .lock()
            .expect("hostname permits should not be poisoned")
            .recv_timeout(Duration::from_secs(2))
            .ok()?;

        Some("printer.lan".to_string())
    }
}

#[test]
fn runtime_streams_hostname_updates_without_waiting_for_lookup() {
    let (started_tx, started_rx) = mpsc::channel();
    let (_permit_tx, permit_rx) = mpsc::channel();
    let resolver = BlockingHostnameResolver {
        started_tx,
        permits: Arc::new(Mutex::new(permit_rx)),
    };

    let task = start_scan_with_probe_and_hostname_resolver(
        "192.168.1.1/32",
        1,
        Arc::new(QuickProbe),
        Arc::new(resolver),
    )
    .unwrap();

    assert!(matches!(
        task.events.recv_timeout(Duration::from_secs(1)),
        Ok(ScanEvent::Started {
            total_hosts: 1,
            ..
        })
    ));
    assert!(matches!(
        task.events.recv_timeout(Duration::from_secs(1)),
        Ok(ScanEvent::HostFound {
            ip,
            mac,
            ..
        }) if ip == "192.168.1.1" && mac == "AA:BB:CC:DD:EE:01"
    ));
    assert!(matches!(
        task.events.recv_timeout(Duration::from_secs(1)),
        Ok(ScanEvent::Progress {
            scanned_hosts: 1,
            total_hosts: 1,
            ..
        })
    ));
    assert!(matches!(
        task.events.recv_timeout(Duration::from_secs(1)),
        Ok(ScanEvent::Finished {
            scanned_hosts: 1,
            found_hosts: 1,
            ..
        })
    ));

    assert_eq!(
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("hostname lookup should have started"),
        "192.168.1.1"
    );
    assert_eq!(
        task.events.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );

    drop(_permit_tx);

    assert!(matches!(
        task.events.recv_timeout(Duration::from_secs(1)),
        Ok(ScanEvent::HostnameResolved {
            ip,
            hostname,
            ..
        }) if ip == "192.168.1.1" && hostname == "printer.lan"
    ));

    task.join_handle
        .join()
        .expect("scan task should exit cleanly");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test scan_runtime -v
```

Expected:

```text
error[E0432]: unresolved import `arp_scan_rs::scan_master::HostnameResolver`
```

or a missing `ScanEvent::HostnameResolved` / missing helper error, depending on which symbol the compiler reaches first.

- [ ] **Step 3: Write minimal implementation**

Add the new resolver module and runtime wiring.

`Cargo.toml`

```toml
[dependencies]
clap = { version = "4.5.53", features = ["derive"] }
dns-lookup = "3.0.1"
slint = "1.14.1"
```

`src/scan_master/hostname.rs`

```rust
use dns_lookup::lookup_addr;
use std::net::IpAddr;

pub trait HostnameResolver: Send + Sync + 'static {
    fn resolve(&self, ip: &str) -> Option<String>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemHostnameResolver;

impl HostnameResolver for SystemHostnameResolver {
    fn resolve(&self, ip: &str) -> Option<String> {
        let addr = ip.parse::<IpAddr>().ok()?;
        lookup_addr(&addr).ok()
    }
}
```

`src/scan_master.rs`

```rust
mod hostname;
mod arp_core;
mod ip_box;
pub mod probe;
pub mod runtime;

pub use hostname::{HostnameResolver, SystemHostnameResolver};
pub use ip_box::{
    first_ip, host_count, host_iter, hosts, next_ip, HostIter, HostMaterializationError,
    HostMaterializeError,
};
pub use arp_core::parse_ip;
pub use probe::{ArpProbe, SystemArpProbe};
pub use runtime::{
    next_task_id, start_scan, start_scan_with_probe_and_hostname_resolver, ScanEvent, ScanTask,
};
```

`src/scan_master/runtime.rs`

```rust
pub enum ScanEvent {
    Started { task_id: u64, total_hosts: usize },
    Progress { task_id: u64, scanned_hosts: usize, total_hosts: usize },
    HostFound { task_id: u64, ip: String, mac: String },
    HostnameResolved { task_id: u64, ip: String, hostname: String },
    Finished { task_id: u64, scanned_hosts: usize, found_hosts: usize },
    Cancelled { task_id: u64, scanned_hosts: usize, found_hosts: usize },
    Failed { task_id: u64, message: String },
}

pub fn start_scan(cidr: &str, max_in_flight: usize) -> io::Result<ScanTask> {
    start_scan_with_probe_and_hostname_resolver(
        cidr,
        max_in_flight,
        Arc::new(SystemArpProbe),
        Arc::new(SystemHostnameResolver),
    )
}

pub fn start_scan_with_probe_and_hostname_resolver<P, H>(
    cidr: &str,
    max_in_flight: usize,
    probe: Arc<P>,
    hostname_resolver: Arc<H>,
) -> io::Result<ScanTask>
where
    P: ArpProbe + Send + Sync + 'static,
    H: HostnameResolver + Send + Sync + 'static,
{
    // same validation and setup as the current start_scan path
    // pass hostname_resolver into run_scan_loop
}

fn spawn_hostname_lookup<H>(
    task_id: u64,
    ip: String,
    hostname_resolver: Arc<H>,
    event_tx: mpsc::Sender<ScanEvent>,
) where
    H: HostnameResolver + Send + Sync + 'static,
{
    thread::spawn(move || {
        if let Some(hostname) = hostname_resolver.resolve(&ip) {
            let _ = event_tx.send(ScanEvent::HostnameResolved {
                task_id,
                ip,
                hostname,
            });
        }
    });
}
```

`src/ui_state.rs`

```rust
ScanEvent::HostnameResolved { task_id, .. } => *task_id,
```

```rust
ScanEvent::HostnameResolved { .. } => ApplyOutcome::applied(RowMutation::None),
```

That compatibility branch is temporary and will be replaced in Task 2.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test scan_runtime -v
```

Expected:

```text
test runtime_streams_hostname_updates_without_waiting_for_lookup ... ok
```

- [ ] **Step 5: Commit**

Run:

```bash
git add Cargo.toml Cargo.lock src/scan_master.rs src/scan_master/hostname.rs src/scan_master/runtime.rs src/ui_state.rs tests/scan_runtime.rs
git commit -m "feat: add reverse dns runtime events"
```

### Task 2: Store Hostnames In The Rust UI State

**Files:**
- Modify: `src/ui_state.rs`
- Modify: `tests/ui_state.rs`

- [ ] **Step 1: Write the failing test**

Add this test to `tests/ui_state.rs`:

```rust
use arp_scan_rs::scan_master::ScanEvent;
use arp_scan_rs::ui_state::ScanUiState;

#[test]
fn ui_state_updates_hostname_in_place_and_ignores_stale_hostname_events() {
    let mut state = ScanUiState::default();

    state.begin_scan(41);
    state.apply_event(ScanEvent::Started {
        task_id: 41,
        total_hosts: 1,
    });
    state.apply_event(ScanEvent::HostFound {
        task_id: 41,
        ip: "192.168.1.10".into(),
        mac: "AA:BB:CC:DD:EE:10".into(),
    });
    state.apply_event(ScanEvent::HostnameResolved {
        task_id: 41,
        ip: "192.168.1.10".into(),
        hostname: "printer.lan".into(),
    });
    state.apply_event(ScanEvent::HostnameResolved {
        task_id: 40,
        ip: "192.168.1.10".into(),
        hostname: "stale.lan".into(),
    });

    let snapshot = state.snapshot();

    assert_eq!(snapshot.rows.len(), 1);
    assert_eq!(snapshot.rows[0].ip, "192.168.1.10");
    assert_eq!(snapshot.rows[0].mac, "AA:BB:CC:DD:EE:10");
    assert_eq!(snapshot.rows[0].hostname, "printer.lan");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test ui_state -v
```

Expected:

```text
error[E0609]: no field `hostname` on type `ResultRow`
```

or a similar compile error pointing at the missing hostname field.

- [ ] **Step 3: Write minimal implementation**

Update the state reducer to store hostnames on rows and update them in place.

`src/ui_state.rs`

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultRow {
    pub ip: String,
    pub mac: String,
    pub hostname: String,
}

// inside apply_event
ScanEvent::HostFound { ip, mac, .. } => {
    let key = ip_to_key(&ip);
    let index = self.results.range(..key).count();
    let row = ResultRow {
        ip,
        mac,
        hostname: String::new(),
    };

    let row_mutation = if self.results.insert(key, row.clone()).is_some() {
        RowMutation::Update { index, row }
    } else {
        RowMutation::Insert { index, row }
    };

    ApplyOutcome::applied(row_mutation)
}
ScanEvent::HostnameResolved { ip, hostname, .. } => {
    let key = ip_to_key(&ip);
    let Some(row) = self.results.get_mut(&key) else {
        return ApplyOutcome::applied(RowMutation::None);
    };

    row.hostname = hostname.clone();
    let index = self.results.range(..key).count();
    ApplyOutcome::applied(RowMutation::Update {
        index,
        row: row.clone(),
    })
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test ui_state -v
```

Expected:

```text
test ui_state_updates_hostname_in_place_and_ignores_stale_hostname_events ... ok
```

- [ ] **Step 5: Commit**

Run:

```bash
git add src/ui_state.rs tests/ui_state.rs
git commit -m "feat: store hostname on scan rows"
```

### Task 3: Expose Hostnames In The Slint Model And Result Panel

**Files:**
- Modify: `ui/main.slint`
- Modify: `src/main.rs`
- Modify: `tests/ui_smoke.rs`
- Modify: `tests/result_list_panel_smoke.rs`

- [ ] **Step 1: Write the failing test**

Update `tests/ui_smoke.rs` to include hostname-bearing rows:

```rust
use std::rc::Rc;

use arp_scan_rs::ui::{MainWindow, ResultListData};
use slint::{Model, ModelRc, VecModel};

#[test]
fn main_window_accepts_sample_result_rows() {
    i_slint_backend_testing::init_no_event_loop();

    let window = MainWindow::new().unwrap();
    let rows = Rc::new(VecModel::from(vec![
        ResultListData {
            ip: "192.168.1.2".into(),
            mac: "AA:BB:CC:DD:EE:02".into(),
        },
        ResultListData {
            ip: "192.168.1.20".into(),
            mac: "AA:BB:CC:DD:EE:20".into(),
        },
    ]));

    window.set_result_list_data_model(ModelRc::from(rows));

    assert_eq!(window.get_result_list_data_model().row_count(), 2);
}
```

Update `tests/result_list_panel_smoke.rs` to expect a hostname field on the row model:

```rust
use std::rc::Rc;

use arp_scan_rs::ui::{ResultListData, ResultListPanel};
use slint::{Model, ModelRc, VecModel};

#[test]
fn result_list_panel_accepts_sample_rows() {
    i_slint_backend_testing::init_no_event_loop();

    let panel = ResultListPanel::new().unwrap();
    let rows = Rc::new(VecModel::from(vec![
        ResultListData {
            ip: "192.168.1.1".into(),
            mac: "74:30:AF:9D:ED:50".into(),
        },
        ResultListData {
            ip: "192.168.1.22".into(),
            mac: "F8:83:06:43:7E:9F".into(),
        },
    ]));

    panel.set_rows(ModelRc::from(rows));

    assert_eq!(panel.get_rows().row_count(), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test ui_smoke --test result_list_panel_smoke -v
```

Expected:

```text
error[E0560]: struct `ResultListData` has no field named `hostname`
```

or a similar compile error in the Slint-generated bindings.

- [ ] **Step 3: Write minimal implementation**

Expose hostname through the Slint model and the Rust-to-Slint mapping, then render it in the result panel.

`ui/main.slint`

```slint
export struct ResultListData {
    ip: string,
    mac: string,
    hostname: string,
}
```

```slint
// inside ResultListPanel rows
HorizontalLayout {
    spacing: root.column-spacing;

    Rectangle { /* IP cell */ }
    Rectangle { /* MAC cell */ }
    Rectangle {
        horizontal-stretch: 1;
        clip: true;

        Text {
            x: 6px;
            width: parent.width - self.x;
            height: 100%;
            text: row.hostname;
            color: #1f2933;
            font-size: root.row-font-size;
            vertical-alignment: center;
            overflow: elide;
        }
    }
}
```

`src/main.rs`

```rust
fn map_row(row: arp_scan_rs::ui_state::ResultRow) -> ResultListData {
    ResultListData {
        ip: row.ip.into(),
        mac: row.mac.into(),
        hostname: row.hostname.into(),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test ui_smoke --test result_list_panel_smoke -v
cargo check
cargo test -v
```

Expected:

```text
test result: ok.
```

The build will still print the existing Slint export warning from Task 2 unless that branch is later refactored, but the tests must pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add ui/main.slint src/main.rs tests/ui_smoke.rs tests/result_list_panel_smoke.rs
git commit -m "feat: show hostname in scan results"
```
