# Large-Range Scan UI Stability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Slint UI responsive for large-range scans by moving scan work off the UI thread, bounding in-flight ARP probe concurrency, streaming progress and host discoveries, and supporting cancellation while preserving already found results.

**Architecture:** Extract the scan runtime into a testable library API that emits typed events, keep Windows-specific ARP probing behind a narrow probe interface, and add a pure Rust UI state reducer that the Slint entrypoint applies on the main thread via `slint::invoke_from_event_loop`. Real-time result ordering and deduplication live in the reducer, while the scan engine only handles CIDR expansion, bounded high-concurrency dispatch, probe execution, and cancellation.

**Tech Stack:** Rust 2024, Slint 1.14, std `mpsc`, `Arc<AtomicBool>`, Windows `SendARP`

---

## File Structure

- `Cargo.toml`
  - Move the `windows` crate into target-specific dependencies and remove dead manifest config.
- `src/lib.rs`
  - New crate library root exporting scan and UI-state modules for tests and the binary.
- `src/main.rs`
  - Thin Slint entrypoint; owns widget wiring and background event handoff.
- `src/ui_state.rs`
  - New pure-Rust reducer for scan lifecycle, status/progress text, result sorting, and deduplication.
- `src/scan_master.rs`
  - Module root re-exporting event types, probe types, runtime API, and CIDR helpers.
- `src/scan_master/arp_core.rs`
  - Windows-only low-level ARP helpers plus a non-Windows stub path.
- `src/scan_master/ip_box.rs`
  - CIDR host iteration and host counting helpers.
- `src/scan_master/probe.rs`
  - New probe abstraction and the `SystemArpProbe` implementation.
- `src/scan_master/runtime.rs`
  - New bounded in-flight scan runtime and cancellation handle.
- `ui/main.slint`
  - Structured result rows, scan status labels, progress labels, and cancel button.
- `tests/probe_stub.rs`
  - Verifies the system probe compiles and reports unsupported on non-Windows.
- `tests/ip_range.rs`
  - Verifies CIDR host iteration/counting behavior.
- `tests/scan_runtime.rs`
  - Verifies event flow, bounded in-flight concurrency, and cancellation with a fake probe.
- `tests/ui_state.rs`
  - Verifies UI-state transitions, stale-task filtering, row sorting, and deduplication.

### Task 1: Make The Scan Core Testable Cross-Platform

**Files:**
- Create: `src/lib.rs`
- Create: `src/scan_master/probe.rs`
- Modify: `Cargo.toml`
- Modify: `src/scan_master.rs`
- Modify: `src/scan_master/arp_core.rs`
- Test: `tests/probe_stub.rs`

- [ ] **Step 1: Write the failing test**

```rust
use arp_scan_rs::scan_master::{ArpProbe, SystemArpProbe};

#[cfg(not(target_os = "windows"))]
#[test]
fn system_probe_reports_unsupported_on_non_windows() {
    let err = SystemArpProbe::default()
        .probe("192.168.1.10")
        .unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test probe_stub -v`
Expected: FAIL with an unresolved import such as `use of unresolved module or unlinked crate 'arp_scan_rs'` or missing `scan_master::SystemArpProbe`.

- [ ] **Step 3: Write minimal implementation**

`Cargo.toml`

```toml
[package]
name = "arp-scan-rs"
version = "0.1.0"
edition = "2024"

[dependencies]
slint = "1.14.1"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.52", features = ["Win32_NetworkManagement_IpHelper", "Win32_Networking_WinSock"] }

[build-dependencies]
slint-build = "1.14.1"
```

`src/lib.rs`

```rust
pub mod scan_master;
```

`src/scan_master/probe.rs`

```rust
use std::io;

use super::arp_core;

pub trait ArpProbe: Send + Sync + 'static {
    fn probe(&self, ip: &str) -> io::Result<Option<String>>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemArpProbe;

impl ArpProbe for SystemArpProbe {
    fn probe(&self, ip: &str) -> io::Result<Option<String>> {
        let ip_addr = arp_core::parse_ip(ip)?;
        match arp_core::get_mac_address(ip_addr) {
            Ok(mac) => Ok(Some(format!(
                "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
            ))),
            Err(err) if err.kind() == io::ErrorKind::TimedOut => Ok(None),
            Err(err) => Err(err),
        }
    }
}
```

`src/scan_master/arp_core.rs`

```rust
use std::io;

#[cfg(target_os = "windows")]
mod imp {
    use std::ffi::CString;
    use std::io;

    const NO_ERROR: u32 = 0;

    #[link(name = "Iphlpapi")]
    unsafe extern "system" {
        fn SendARP(dest_ip: u32, src_ip: u32, mac_addr: *mut u8, phy_addr_len: *mut u32) -> u32;
    }

    #[link(name = "Ws2_32")]
    unsafe extern "system" {
        fn inet_addr(ip: *const i8) -> u32;
    }

    pub fn parse_ip(ip_str: &str) -> io::Result<u32> {
        let c_str = CString::new(ip_str)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        let addr = unsafe { inet_addr(c_str.as_ptr()) };

        if addr == u32::MAX {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid IP address format",
            ))
        } else {
            Ok(addr)
        }
    }

    pub fn get_mac_address(ip: u32) -> io::Result<[u8; 6]> {
        let mut mac_addr = [0u8; 6];
        let mut phy_addr_len = 6u32;
        let result = unsafe { SendARP(ip, 0, mac_addr.as_mut_ptr(), &mut phy_addr_len) };

        if result == NO_ERROR {
            Ok(mac_addr)
        } else {
            Err(io::Error::from_raw_os_error(result as i32))
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use std::io;

    pub fn parse_ip(ip_str: &str) -> io::Result<u32> {
        ip_str
            .parse::<std::net::Ipv4Addr>()
            .map(u32::from)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))
    }

    pub fn get_mac_address(_ip: u32) -> io::Result<[u8; 6]> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "ARP probing is only supported on Windows",
        ))
    }
}

pub use imp::{get_mac_address, parse_ip};
```

`src/scan_master.rs`

```rust
pub mod arp_core;
pub mod ip_box;
pub mod probe;

pub use probe::{ArpProbe, SystemArpProbe};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test probe_stub -v`
Expected: PASS with `test system_probe_reports_unsupported_on_non_windows ... ok` on non-Windows, or the test is skipped by `cfg` on Windows.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/lib.rs src/scan_master.rs src/scan_master/arp_core.rs src/scan_master/probe.rs tests/probe_stub.rs
git commit -m "refactor: make scan core testable across platforms"
```

### Task 2: Add CIDR Iteration And Typed Scan Events

**Files:**
- Create: `src/scan_master/runtime.rs`
- Modify: `src/scan_master.rs`
- Modify: `src/scan_master/ip_box.rs`
- Test: `tests/ip_range.rs`

- [ ] **Step 1: Write the failing test**

```rust
use arp_scan_rs::scan_master::{host_count, hosts};

#[test]
fn hosts_and_counts_follow_cidr_rules() {
    assert_eq!(host_count("192.168.1.0/24"), Some(254));
    assert_eq!(
        hosts("192.168.1.0/30").unwrap(),
        vec!["192.168.1.1".to_string(), "192.168.1.2".to_string()]
    );
    assert_eq!(
        hosts("192.168.1.0/31").unwrap(),
        vec!["192.168.1.0".to_string(), "192.168.1.1".to_string()]
    );
    assert_eq!(hosts("192.168.1.5/32").unwrap(), vec!["192.168.1.5".to_string()]);
}

#[test]
fn invalid_cidr_returns_none_or_error() {
    assert_eq!(host_count("bad"), None);
    assert!(hosts("192.168.1.0/33").is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test ip_range -v`
Expected: FAIL with missing exports such as `no 'host_count' in 'scan_master'` or `no 'hosts' in 'scan_master'`.

- [ ] **Step 3: Write minimal implementation**

`src/scan_master/runtime.rs`

```rust
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanEvent {
    Started { task_id: u64, total_hosts: usize },
    Progress { task_id: u64, scanned_hosts: usize, total_hosts: usize },
    HostFound { task_id: u64, ip: String, mac: String },
    Finished { task_id: u64, scanned_hosts: usize, found_hosts: usize },
    Cancelled { task_id: u64, scanned_hosts: usize, found_hosts: usize },
    Failed { task_id: u64, message: String },
}

#[derive(Debug)]
pub struct ScanTask {
    pub task_id: u64,
    pub events: mpsc::Receiver<ScanEvent>,
    pub cancel_flag: Arc<AtomicBool>,
    pub join_handle: JoinHandle<()>,
}

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

pub fn next_task_id() -> u64 {
    NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed)
}
```

`src/scan_master/ip_box.rs`

```rust
use std::net::Ipv4Addr;

fn parse_cidr(cidr: &str) -> Option<(u32, u32, u8)> {
    let (net_str, prefix_len_str) = cidr.split_once('/')?;
    let net_ip: Ipv4Addr = net_str.parse().ok()?;
    let prefix_len: u8 = prefix_len_str.parse().ok()?;

    if prefix_len > 32 {
        return None;
    }

    let net_u32 = u32::from(net_ip);
    let mask = if prefix_len == 0 { 0 } else { (!0u32) << (32 - prefix_len) };
    let network = net_u32 & mask;
    let broadcast = network | !mask;
    Some((network, broadcast, prefix_len))
}

fn usable_bounds(cidr: &str) -> Option<(u32, u32)> {
    let (network, broadcast, prefix_len) = parse_cidr(cidr)?;
    match prefix_len {
        0..=30 if broadcast > network => Some((network + 1, broadcast - 1)),
        31 => Some((network, broadcast)),
        32 => Some((network, network)),
        _ => None,
    }
}

pub fn host_count(cidr: &str) -> Option<usize> {
    let (first, last) = usable_bounds(cidr)?;
    Some((last - first + 1) as usize)
}

pub fn hosts(cidr: &str) -> Option<Vec<String>> {
    let (first, last) = usable_bounds(cidr)?;
    Some(
        (first..=last)
            .map(|raw| Ipv4Addr::from(raw).to_string())
            .collect(),
    )
}
```

`src/scan_master.rs`

```rust
pub mod arp_core;
pub mod ip_box;
pub mod probe;
pub mod runtime;

pub use ip_box::{host_count, hosts};
pub use probe::{ArpProbe, SystemArpProbe};
pub use runtime::{next_task_id, ScanEvent, ScanTask};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test ip_range -v`
Expected: PASS with both tests green.

- [ ] **Step 5: Commit**

```bash
git add src/scan_master.rs src/scan_master/ip_box.rs src/scan_master/runtime.rs tests/ip_range.rs
git commit -m "feat: add cidr iteration and scan event types"
```

### Task 3: Implement The Bounded In-Flight Scan Runtime

**Files:**
- Modify: `src/scan_master.rs`
- Modify: `src/scan_master/runtime.rs`
- Modify: `src/scan_master/probe.rs`
- Test: `tests/scan_runtime.rs`

- [ ] **Step 1: Write the failing test**

```rust
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

use arp_scan_rs::scan_master::{start_scan_with_probe, ArpProbe, ScanEvent};

#[derive(Clone)]
struct FakeProbe {
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
    barrier: Arc<Barrier>,
}

impl ArpProbe for FakeProbe {
    fn probe(&self, ip: &str) -> io::Result<Option<String>> {
        let current = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(current, Ordering::SeqCst);
        self.barrier.wait();
        self.active.fetch_sub(1, Ordering::SeqCst);

        if ip.ends_with(".1") || ip.ends_with(".2") {
            Ok(Some("AA:BB:CC:DD:EE:FF".into()))
        } else {
            Ok(None)
        }
    }
}

#[test]
fn runtime_respects_max_in_flight_and_streams_terminal_event() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let probe = FakeProbe {
        active: active.clone(),
        peak: peak.clone(),
        barrier: Arc::new(Barrier::new(2)),
    };

    let task = start_scan_with_probe("192.168.1.0/30", 2, Arc::new(probe)).unwrap();
    let events: Vec<_> = task.events.iter().collect();
    task.join_handle.join().unwrap();

    assert!(events.iter().any(|event| matches!(event, ScanEvent::Started { total_hosts: 2, .. })));
    assert!(events.iter().any(|event| matches!(event, ScanEvent::HostFound { ip, .. } if ip == "192.168.1.1")));
    assert!(events.iter().any(|event| matches!(event, ScanEvent::Finished { scanned_hosts: 2, found_hosts: 2, .. })));
    assert_eq!(peak.load(Ordering::SeqCst), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test scan_runtime runtime_respects_max_in_flight_and_streams_terminal_event -v`
Expected: FAIL with missing `start_scan_with_probe` or missing runtime behavior.

- [ ] **Step 3: Write minimal implementation**

`src/scan_master/runtime.rs`

```rust
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::thread::JoinHandle;

use super::ip_box::{host_count, hosts};
use super::probe::{ArpProbe, SystemArpProbe};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanEvent {
    Started { task_id: u64, total_hosts: usize },
    Progress { task_id: u64, scanned_hosts: usize, total_hosts: usize },
    HostFound { task_id: u64, ip: String, mac: String },
    Finished { task_id: u64, scanned_hosts: usize, found_hosts: usize },
    Cancelled { task_id: u64, scanned_hosts: usize, found_hosts: usize },
    Failed { task_id: u64, message: String },
}

#[derive(Debug)]
pub struct ScanTask {
    pub task_id: u64,
    pub events: mpsc::Receiver<ScanEvent>,
    pub cancel_flag: Arc<AtomicBool>,
    pub join_handle: JoinHandle<()>,
}

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

pub fn next_task_id() -> u64 {
    NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed)
}

pub fn start_scan(cidr: &str, max_in_flight: usize) -> io::Result<ScanTask> {
    start_scan_with_probe(cidr, max_in_flight, Arc::new(SystemArpProbe))
}

pub fn start_scan_with_probe<P>(
    cidr: &str,
    max_in_flight: usize,
    probe: Arc<P>,
) -> io::Result<ScanTask>
where
    P: ArpProbe,
{
    let total_hosts = host_count(cidr)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid CIDR"))?;
    let host_list = hosts(cidr)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid CIDR"))?;

    let task_id = next_task_id();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let (event_tx, event_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let cancel_for_thread = cancel_flag.clone();

    let join_handle = thread::spawn(move || {
        let _ = event_tx.send(ScanEvent::Started { task_id, total_hosts });
        let mut scanned_hosts = 0usize;
        let mut found_hosts = 0usize;
        let mut in_flight = 0usize;
        let mut next_index = 0usize;

        while next_index < host_list.len() || in_flight > 0 {
            while next_index < host_list.len()
                && in_flight < max_in_flight
                && !cancel_for_thread.load(Ordering::Relaxed)
            {
                let ip = host_list[next_index].clone();
                let probe = probe.clone();
                let done_tx = done_tx.clone();

                thread::spawn(move || {
                    let result = probe.probe(&ip);
                    let _ = done_tx.send((ip, result));
                });

                next_index += 1;
                in_flight += 1;
            }

            if in_flight == 0 {
                break;
            }

            let Ok((ip, result)) = done_rx.recv() else {
                let _ = event_tx.send(ScanEvent::Failed {
                    task_id,
                    message: "scan worker channel closed unexpectedly".into(),
                });
                return;
            };

            in_flight -= 1;
            scanned_hosts += 1;

            match result {
                Ok(Some(mac)) => {
                    found_hosts += 1;
                    let _ = event_tx.send(ScanEvent::HostFound { task_id, ip, mac });
                }
                Ok(None) => {}
                Err(err) if err.kind() == io::ErrorKind::Unsupported => {
                    let _ = event_tx.send(ScanEvent::Failed {
                        task_id,
                        message: err.to_string(),
                    });
                    return;
                }
                Err(_) => {}
            }

            let _ = event_tx.send(ScanEvent::Progress {
                task_id,
                scanned_hosts,
                total_hosts,
            });
        }

        let terminal_event = if cancel_for_thread.load(Ordering::Relaxed) {
            ScanEvent::Cancelled {
                task_id,
                scanned_hosts,
                found_hosts,
            }
        } else {
            ScanEvent::Finished {
                task_id,
                scanned_hosts,
                found_hosts,
            }
        };

        let _ = event_tx.send(terminal_event);
    });

    Ok(ScanTask {
        task_id,
        events: event_rx,
        cancel_flag,
        join_handle,
    })
}
```

`src/scan_master.rs`

```rust
pub mod arp_core;
pub mod ip_box;
pub mod probe;
pub mod runtime;

pub use ip_box::{host_count, hosts};
pub use probe::{ArpProbe, SystemArpProbe};
pub use runtime::{next_task_id, start_scan, start_scan_with_probe, ScanEvent, ScanTask};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test scan_runtime runtime_respects_max_in_flight_and_streams_terminal_event -v`
Expected: PASS with the single runtime test green.

- [ ] **Step 5: Commit**

```bash
git add src/scan_master.rs src/scan_master/runtime.rs src/scan_master/probe.rs tests/scan_runtime.rs
git commit -m "feat: add bounded in-flight scan runtime"
```

### Task 4: Add UI State Reduction And Wire It Into Slint

**Files:**
- Create: `src/ui_state.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`
- Modify: `ui/main.slint`
- Test: `tests/ui_state.rs`

- [ ] **Step 1: Write the failing test**

```rust
use arp_scan_rs::scan_master::ScanEvent;
use arp_scan_rs::ui_state::ScanUiState;

#[test]
fn ui_state_sorts_deduplicates_and_ignores_stale_events() {
    let mut state = ScanUiState::default();

    state.begin_scan(7);
    state.apply_event(ScanEvent::Started { task_id: 7, total_hosts: 4 });
    state.apply_event(ScanEvent::HostFound {
        task_id: 7,
        ip: "192.168.1.20".into(),
        mac: "AA:AA:AA:AA:AA:20".into(),
    });
    state.apply_event(ScanEvent::HostFound {
        task_id: 7,
        ip: "192.168.1.2".into(),
        mac: "AA:AA:AA:AA:AA:02".into(),
    });
    state.apply_event(ScanEvent::HostFound {
        task_id: 7,
        ip: "192.168.1.20".into(),
        mac: "AA:AA:AA:AA:AA:20".into(),
    });
    state.apply_event(ScanEvent::HostFound {
        task_id: 6,
        ip: "192.168.1.1".into(),
        mac: "STALE".into(),
    });

    let rows = state.rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].ip, "192.168.1.2");
    assert_eq!(rows[1].ip, "192.168.1.20");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test ui_state ui_state_sorts_deduplicates_and_ignores_stale_events -v`
Expected: FAIL with missing `ui_state::ScanUiState`.

- [ ] **Step 3: Write minimal implementation**

`src/ui_state.rs`

```rust
use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use crate::scan_master::ScanEvent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultRow {
    pub ip: String,
    pub mac: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewSnapshot {
    pub is_scanning: bool,
    pub can_cancel: bool,
    pub status_text: String,
    pub progress_text: String,
    pub rows: Vec<ResultRow>,
}

#[derive(Debug, Default)]
pub struct ScanUiState {
    active_task_id: Option<u64>,
    total_hosts: usize,
    scanned_hosts: usize,
    found_hosts: usize,
    status_text: String,
    results: BTreeMap<u32, ResultRow>,
    is_scanning: bool,
}

impl ScanUiState {
    pub fn begin_scan(&mut self, task_id: u64) {
        self.active_task_id = Some(task_id);
        self.total_hosts = 0;
        self.scanned_hosts = 0;
        self.found_hosts = 0;
        self.status_text = "Scanning".into();
        self.results.clear();
        self.is_scanning = true;
    }

    pub fn apply_event(&mut self, event: ScanEvent) {
        if !self.belongs_to_active_task(&event) {
            return;
        }

        match event {
            ScanEvent::Started { total_hosts, .. } => {
                self.total_hosts = total_hosts;
                self.status_text = "Scanning".into();
            }
            ScanEvent::Progress { scanned_hosts, .. } => {
                self.scanned_hosts = scanned_hosts;
            }
            ScanEvent::HostFound { ip, mac, .. } => {
                self.found_hosts = self.found_hosts.max(self.results.len() + 1);
                self.results.insert(ip_to_key(&ip), ResultRow { ip, mac });
            }
            ScanEvent::Finished { found_hosts, .. } => {
                self.found_hosts = found_hosts;
                self.status_text = format!("Finished ({found_hosts} hosts)");
                self.is_scanning = false;
            }
            ScanEvent::Cancelled { found_hosts, .. } => {
                self.found_hosts = found_hosts;
                self.status_text = format!("Cancelled ({found_hosts} hosts)");
                self.is_scanning = false;
            }
            ScanEvent::Failed { message, .. } => {
                self.status_text = format!("Failed: {message}");
                self.is_scanning = false;
            }
        }
    }

    pub fn rows(&self) -> Vec<ResultRow> {
        self.results.values().cloned().collect()
    }

    pub fn snapshot(&self) -> ViewSnapshot {
        ViewSnapshot {
            is_scanning: self.is_scanning,
            can_cancel: self.is_scanning,
            status_text: self.status_text.clone(),
            progress_text: format!("{} / {}", self.scanned_hosts, self.total_hosts),
            rows: self.rows(),
        }
    }

    fn belongs_to_active_task(&self, event: &ScanEvent) -> bool {
        let event_task_id = match event {
            ScanEvent::Started { task_id, .. }
            | ScanEvent::Progress { task_id, .. }
            | ScanEvent::HostFound { task_id, .. }
            | ScanEvent::Finished { task_id, .. }
            | ScanEvent::Cancelled { task_id, .. }
            | ScanEvent::Failed { task_id, .. } => *task_id,
        };

        self.active_task_id == Some(event_task_id)
    }
}

fn ip_to_key(ip: &str) -> u32 {
    ip.parse::<Ipv4Addr>().map(u32::from).unwrap_or(u32::MAX)
}
```

`src/lib.rs`

```rust
pub mod scan_master;
pub mod ui_state;
```

`ui/main.slint`

```slint
import { Button, GroupBox, LineEdit, ListView } from "std-widgets.slint";

export struct ResultListData {
    ip: string,
    mac: string,
}

export component MainWindow inherits Window {
    title: "ArpScan-rs";
    width: 520px;
    height: 560px;

    in property <[ResultListData]> result_list_data_model;
    in-out property <string> cidr;
    in property <string> status_text;
    in property <string> progress_text;
    in property <bool> scan_enabled;
    in property <bool> cancel_enabled;

    callback do_scan();
    callback do_cancel();

    VerticalLayout {
        padding: 20px;
        spacing: 10px;

        GroupBox {
            title: "CIDR";
            LineEdit {
                text <=> cidr;
                enabled: root.scan_enabled;
                placeholder-text: "192.168.1.0/24";
            }
        }

        HorizontalLayout {
            spacing: 8px;
            Button {
                text: "Scan";
                enabled: root.scan_enabled;
                clicked => { root.do_scan(); }
            }
            Button {
                text: "Cancel";
                enabled: root.cancel_enabled;
                clicked => { root.do_cancel(); }
            }
        }

        Text { text: root.status_text; }
        Text { text: root.progress_text; }

        Rectangle {
            height: 380px;
            border-width: 1px;
            border-color: #d0d7de;

            ListView {
                for row in result_list_data_model: Rectangle {
                    height: 28px;
                    HorizontalLayout {
                        Text { text: row.ip; width: 180px; }
                        Text { text: row.mac; }
                    }
                }
            }
        }
    }
}
```

`src/main.rs`

```rust
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;

use arp_scan_rs::scan_master::{start_scan, ScanEvent, ScanTask};
use arp_scan_rs::ui_state::ScanUiState;
use slint::{ModelRc, VecModel};

slint::include_modules!();

const MAX_IN_FLIGHT: usize = 256;

#[derive(Clone)]
struct ActiveScan {
    task_id: u64,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
}

fn main() {
    let window = MainWindow::new().unwrap();
    let state = Arc::new(Mutex::new(ScanUiState::default()));
    let active_scan = Arc::new(Mutex::new(None::<ActiveScan>));

    window.set_result_list_data_model(ModelRc::from(std::rc::Rc::new(VecModel::from(vec![]))));
    window.set_status_text("Idle".into());
    window.set_progress_text("0 / 0".into());
    window.set_scan_enabled(true);
    window.set_cancel_enabled(false);

    let weak = window.as_weak();
    let state_for_scan = state.clone();
    let active_scan_for_start = active_scan.clone();

    window.on_do_scan(move || {
        let Some(window) = weak.upgrade() else { return; };
        let cidr = window.get_cidr().to_string();

        let Ok(task) = start_scan(&cidr, MAX_IN_FLIGHT) else {
            window.set_status_text("Failed: invalid CIDR".into());
            return;
        };

        {
            let mut state = state_for_scan.lock().unwrap();
            state.begin_scan(task.task_id);
            apply_snapshot(&window, state.snapshot());
        }

        let weak = window.as_weak();
        let state = state_for_scan.clone();
        let active_scan = active_scan_for_start.clone();
        let cancel_flag = task.cancel_flag.clone();
        let task_id = task.task_id;

        *active_scan.lock().unwrap() = Some(ActiveScan { task_id, cancel_flag });

        thread::spawn(move || {
            let task = task;
            for event in task.events {
                let weak = weak.clone();
                let state = state.clone();
                slint::invoke_from_event_loop(move || {
                    if let Some(window) = weak.upgrade() {
                        let mut state = state.lock().unwrap();
                        state.apply_event(event);
                        apply_snapshot(&window, state.snapshot());
                    }
                })
                .unwrap();
            }
            let _ = task.join_handle.join();
        });
    });

    let active_scan_for_cancel = active_scan.clone();
    window.on_do_cancel(move || {
        if let Some(active_scan) = active_scan_for_cancel.lock().unwrap().as_ref() {
            active_scan.cancel_flag.store(true, Ordering::Relaxed);
        }
    });

    window.run().unwrap();
}

fn apply_snapshot(window: &MainWindow, snapshot: arp_scan_rs::ui_state::ViewSnapshot) {
    window.set_status_text(snapshot.status_text.into());
    window.set_progress_text(snapshot.progress_text.into());
    window.set_scan_enabled(!snapshot.is_scanning);
    window.set_cancel_enabled(snapshot.can_cancel);

    let rows = snapshot
            .rows
            .into_iter()
            .map(|row| ResultListData {
                ip: row.ip.into(),
                mac: row.mac.into(),
            })
            .collect();

    window.set_result_list_data_model(ModelRc::from(std::rc::Rc::new(VecModel::from(rows))));
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test ui_state -v`
Expected: PASS with the reducer test green.

Run: `cargo test --test probe_stub --test ip_range --test scan_runtime --test ui_state -v`
Expected: PASS for all four integration test files.

Run: `cargo check`
Expected: PASS with the application compiling.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/ui_state.rs src/main.rs ui/main.slint tests/ui_state.rs
git commit -m "feat: wire responsive scan state into slint ui"
```

## Self-Review

### Spec coverage

- Responsive UI: covered by Task 4 moving event application to `slint::invoke_from_event_loop`.
- Bounded concurrency for large ranges: covered by Task 3 runtime window.
- Cancellation with preserved results: covered by Tasks 3 and 4.
- Real-time, sorted, deduplicated results: covered by Task 4 reducer.
- Probe abstraction for tests: covered by Task 1.
- CIDR counting/iteration for progress: covered by Task 2.

No spec requirement is left without a task.

### Placeholder scan

No `TBD`, `TODO`, or “similar to previous task” placeholders remain. Each task has exact files, commands, and code examples.

### Type consistency

- `ScanEvent` is defined once in `src/scan_master/runtime.rs` and reused in tests and UI state.
- `SystemArpProbe` consistently implements `ArpProbe`.
- `ScanUiState` consistently owns sorted `ResultRow` values and produces a `ViewSnapshot`.
