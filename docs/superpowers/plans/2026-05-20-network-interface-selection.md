# Network Interface Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add network interface enumeration to the UI, keep all interfaces visible with unavailable ones labeled, make unavailable entries truly unselectable, and autofill the existing scan target inputs from the selected usable interface without changing the active input mode.

**Architecture:** Introduce a focused `network_interface` module that projects platform data into a UI-friendly list of interface items. Reuse `scan_target` as the single place that knows how to apply interface-derived IPv4 data to the current input state, then extend `main.rs` and Slint bindings so the UI can show the list, keep a default empty selection, and update the visible target fields when the user chooses a usable interface. Replace the plain `ComboBox` with a custom popup selector so each row can expose its own enabled state while staying visible.

**Tech Stack:** Rust 2024, Slint 1.14, cargo test, Windows IP Helper API, existing `scan_target` conversion helpers

---

## File Structure

- Create: `src/network_interface.rs`
  - Own platform-neutral interface item types, availability rules, display-label formatting, Windows enumeration, and non-Windows fallback behavior.
- Modify: `src/lib.rs`
  - Export the new `network_interface` module.
- Modify: `src/scan_target.rs`
  - Add helpers that apply a selected interface’s IPv4 data to the existing `UiInputState` while preserving the current mode.
- Modify: `ui/main.slint`
  - Add interface-selector properties and a custom selector row in the scan target group.
- Modify: `src/main.rs`
  - Load interface items, expose them to Slint, initialize the empty selection, and handle selection-driven autofill.
- Modify: `tests/scan_target.rs`
  - Cover interface-driven autofill behavior in both input modes.
- Modify: `tests/ui_smoke.rs`
  - Cover the new Slint bindings and default empty selection.
- Create: `tests/network_interface.rs`
  - Cover interface item availability, labels, and non-Windows fallback behavior.

### Task 1: Add failing network-interface model tests

**Files:**
- Create: `tests/network_interface.rs`
- Modify: `src/lib.rs`
- Create: `src/network_interface.rs`
- Test: `tests/network_interface.rs`

- [ ] **Step 1: Write the failing test**

```rust
use arp_scan_rs::network_interface::{
    format_interface_label, load_network_interfaces, NetworkInterface, NetworkInterfaceAvailability,
    NetworkInterfaceIpv4,
};

#[test]
fn available_interface_keeps_plain_display_label() {
    let item = NetworkInterface {
        id: "ethernet-1".into(),
        name: "Ethernet".into(),
        availability: NetworkInterfaceAvailability::Available(NetworkInterfaceIpv4 {
            ip: "192.168.1.23".into(),
            mask: "255.255.255.0".into(),
            cidr: "192.168.1.0/24".into(),
        }),
    };

    assert_eq!(format_interface_label(&item), "Ethernet");
}

#[test]
fn unavailable_interface_appends_reason_to_display_label() {
    let item = NetworkInterface {
        id: "loopback".into(),
        name: "Loopback Pseudo-Interface".into(),
        availability: NetworkInterfaceAvailability::Unavailable {
            reason: "Loopback".into(),
        },
    };

    assert_eq!(
        format_interface_label(&item),
        "Loopback Pseudo-Interface (Loopback)"
    );
}

#[test]
fn load_network_interfaces_returns_at_least_placeholder_entries_that_have_labels() {
    let interfaces = load_network_interfaces().expect("interface load should not fail hard");

    assert!(
        !interfaces.is_empty(),
        "the UI needs either real interfaces or a placeholder entry"
    );
    assert!(
        interfaces
            .iter()
            .all(|item| !format_interface_label(item).trim().is_empty()),
        "every interface item should have a visible label"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test network_interface -v`
Expected: FAIL with unresolved import errors for `arp_scan_rs::network_interface`

- [ ] **Step 3: Write minimal implementation**

`src/lib.rs`

```rust
pub mod network_interface;
pub mod scan_master;
pub mod scan_target;
pub mod ui;
pub mod ui_state;
```

`src/network_interface.rs`

```rust
use std::io;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkInterface {
    pub id: String,
    pub name: String,
    pub availability: NetworkInterfaceAvailability,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkInterfaceAvailability {
    Available(NetworkInterfaceIpv4),
    Unavailable { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkInterfaceIpv4 {
    pub ip: String,
    pub mask: String,
    pub cidr: String,
}

pub fn format_interface_label(item: &NetworkInterface) -> String {
    match &item.availability {
        NetworkInterfaceAvailability::Available(_) => item.name.clone(),
        NetworkInterfaceAvailability::Unavailable { reason } => {
            format!("{} ({reason})", item.name)
        }
    }
}

pub fn load_network_interfaces() -> io::Result<Vec<NetworkInterface>> {
    #[cfg(target_os = "windows")]
    {
        load_windows_interfaces()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(vec![NetworkInterface {
            id: "unsupported-platform".into(),
            name: "This platform".into(),
            availability: NetworkInterfaceAvailability::Unavailable {
                reason: "Unsupported platform".into(),
            },
        }])
    }
}

#[cfg(target_os = "windows")]
fn load_windows_interfaces() -> io::Result<Vec<NetworkInterface>> {
    Ok(vec![NetworkInterface {
        id: "loading-not-implemented".into(),
        name: "Interface loading".into(),
        availability: NetworkInterfaceAvailability::Unavailable {
            reason: "Failed to load interfaces".into(),
        },
    }])
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test network_interface -v`
Expected: PASS with `3 passed`

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/network_interface.rs tests/network_interface.rs
git commit -m "test: 添加网络接口模型覆盖"
```

### Task 2: Add failing interface-autofill tests

**Files:**
- Modify: `tests/scan_target.rs`
- Modify: `src/scan_target.rs`
- Test: `tests/scan_target.rs`

- [ ] **Step 1: Write the failing test**

Add these tests to `tests/scan_target.rs`:

```rust
use arp_scan_rs::network_interface::{NetworkInterfaceAvailability, NetworkInterfaceIpv4};
use arp_scan_rs::scan_target::{apply_interface_to_input_state, InputMode, UiInputState};

#[test]
fn usable_interface_updates_ip_and_mask_when_mode_is_ip_and_mask() {
    let state = UiInputState {
        mode: InputMode::IpAndMask,
        cidr_text: "10.0.0.0/8".into(),
        ip_text: "10.0.0.8".into(),
        mask_text: "255.0.0.0".into(),
    };

    let updated = apply_interface_to_input_state(
        &state,
        &NetworkInterfaceAvailability::Available(NetworkInterfaceIpv4 {
            ip: "192.168.1.23".into(),
            mask: "255.255.255.0".into(),
            cidr: "192.168.1.0/24".into(),
        }),
    );

    assert_eq!(updated.mode, InputMode::IpAndMask);
    assert_eq!(updated.ip_text, "192.168.1.23");
    assert_eq!(updated.mask_text, "255.255.255.0");
    assert_eq!(updated.cidr_text, "10.0.0.0/8");
}

#[test]
fn usable_interface_updates_cidr_when_mode_is_cidr() {
    let state = UiInputState {
        mode: InputMode::Cidr,
        cidr_text: "10.0.0.0/8".into(),
        ip_text: "10.0.0.8".into(),
        mask_text: "255.0.0.0".into(),
    };

    let updated = apply_interface_to_input_state(
        &state,
        &NetworkInterfaceAvailability::Available(NetworkInterfaceIpv4 {
            ip: "192.168.1.23".into(),
            mask: "255.255.255.0".into(),
            cidr: "192.168.1.0/24".into(),
        }),
    );

    assert_eq!(updated.mode, InputMode::Cidr);
    assert_eq!(updated.cidr_text, "192.168.1.0/24");
    assert_eq!(updated.ip_text, "10.0.0.8");
    assert_eq!(updated.mask_text, "255.0.0.0");
}

#[test]
fn unavailable_interface_keeps_existing_values_unchanged() {
    let state = UiInputState {
        mode: InputMode::IpAndMask,
        cidr_text: "10.0.0.0/8".into(),
        ip_text: "10.0.0.8".into(),
        mask_text: "255.0.0.0".into(),
    };

    let updated = apply_interface_to_input_state(
        &state,
        &NetworkInterfaceAvailability::Unavailable {
            reason: "No IPv4 address".into(),
        },
    );

    assert_eq!(updated, state);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test scan_target -v`
Expected: FAIL with unresolved import errors for `apply_interface_to_input_state`

- [ ] **Step 3: Write minimal implementation**

Add this helper to `src/scan_target.rs`:

```rust
use crate::network_interface::NetworkInterfaceAvailability;

pub fn apply_interface_to_input_state(
    state: &UiInputState,
    availability: &NetworkInterfaceAvailability,
) -> UiInputState {
    let mut next = state.clone();

    match (state.mode, availability) {
        (InputMode::Cidr, NetworkInterfaceAvailability::Available(ipv4)) => {
            next.cidr_text = ipv4.cidr.clone();
        }
        (InputMode::IpAndMask, NetworkInterfaceAvailability::Available(ipv4)) => {
            next.ip_text = ipv4.ip.clone();
            next.mask_text = ipv4.mask.clone();
        }
        (_, NetworkInterfaceAvailability::Unavailable { .. }) => {}
    }

    next
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test scan_target -v`
Expected: PASS with the existing tests plus `3` new interface-autofill tests

- [ ] **Step 5: Commit**

```bash
git add src/scan_target.rs tests/scan_target.rs
git commit -m "test: 补充网络接口回填输入覆盖"
```

### Task 3: Add failing UI binding and startup-integration tests

**Files:**
- Modify: `ui/main.slint`
- Modify: `src/main.rs`
- Modify: `tests/ui_smoke.rs`
- Modify: `src/network_interface.rs`
- Test: `tests/ui_smoke.rs`

- [ ] **Step 1: Write the failing test**

Replace the single test in `tests/ui_smoke.rs` with:

```rust
use arp_scan_rs::ui::{MainWindow, NetworkInterfaceItem, ResultListData};
use slint::{Model, ModelRc, VecModel};

#[test]
fn main_window_smoke_test_exposes_interface_bindings_and_defaults() {
    let _backend = i_slint_backend_testing::init_no_event_loop();

    let window = MainWindow::new().unwrap();
    let rows = VecModel::from(vec![
        ResultListData {
            ip: "192.168.1.2".into(),
            mac: "AA:AA:AA:AA:AA:02".into(),
            hostname: "printer.local".into(),
        },
        ResultListData {
            ip: "192.168.1.20".into(),
            mac: "AA:AA:AA:AA:AA:14".into(),
            hostname: "".into(),
        },
    ]);
    let interfaces = VecModel::from(vec![
        NetworkInterfaceItem {
            label: "Select network interface".into(),
            enabled: true,
        },
        NetworkInterfaceItem {
            label: "Ethernet".into(),
            enabled: true,
        },
        NetworkInterfaceItem {
            label: "Loopback Pseudo-Interface (Loopback)".into(),
            enabled: false,
        },
    ]);

    window.set_result_list_data_model(ModelRc::from(std::rc::Rc::new(rows)));
    window.set_network_interface_model(ModelRc::from(std::rc::Rc::new(interfaces)));
    window.set_selected_network_interface_index(0);
    window.set_input_mode(0);
    window.set_cidr_text("192.168.1.0/24".into());
    window.set_ip_text("192.168.1.23".into());
    window.set_mask_text("255.255.255.0".into());
    window.set_status_text("Scanning".into());
    window.set_progress_text("2 / 254".into());
    window.set_scan_enabled(false);
    window.set_cancel_enabled(true);

    assert_eq!(window.get_selected_network_interface_index(), 0);
    let interface_model = window.get_network_interface_model();
    let interface_rows = interface_model
        .as_any()
        .downcast_ref::<VecModel<NetworkInterfaceItem>>()
        .unwrap();
    assert_eq!(interface_rows.row_count(), 3);
    assert_eq!(interface_rows.row_data(0).unwrap().label, "Select network interface");
    assert_eq!(interface_rows.row_data(1).unwrap().label, "Ethernet");
    assert!(interface_rows.row_data(1).unwrap().enabled);
    assert_eq!(
        interface_rows.row_data(2).unwrap().label,
        "Loopback Pseudo-Interface (Loopback)"
    );
    assert!(!interface_rows.row_data(2).unwrap().enabled);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test ui_smoke -v`
Expected: FAIL with missing `NetworkInterfaceItem` / `network_interface_model` / `selected_network_interface_index` bindings

- [ ] **Step 3: Write minimal implementation**

`ui/main.slint`

```slint
export struct NetworkInterfaceItem {
    label: string,
    enabled: bool,
}
```

Add these properties and callback to `MainWindow`:

```slint
in property <[NetworkInterfaceItem]> network_interface_model;
in-out property <int> selected_network_interface_index: 0;
callback network_interface_changed(index: int);
```

Add the custom selector row ahead of the mode selector:

```slint
// Trigger text mirrors the selected interface or the placeholder at index 0.
// The popup renders each row and only wires click/keyboard selection when
// `item.enabled` is true.

component NetworkInterfaceSelector inherits Rectangle {
    in property <[NetworkInterfaceItem]> model;
    in-out property <int> current-index: 0;
    in property <bool> enabled: true;
    callback selected(index: int);

    trigger := Button {
        text: root.model[root.current-index].label;
        enabled: root.enabled;
        clicked => { popup.open(); }
    }

    popup := PopupWindow {
        ScrollView {
            VerticalLayout {
                for item[index] in root.model : Rectangle {
                    min-height: 36px;
                    background: item.enabled ? transparent : #f0f2f5;

                    TouchArea {
                        enabled: item.enabled;
                        clicked => {
                            root.current-index = index;
                            root.selected(index);
                            popup.close();
                        }
                    }
                }
            }
        }
    }
}
```

`src/main.rs`

```rust
use arp_scan_rs::network_interface::{
    format_interface_label, load_network_interfaces, NetworkInterfaceAvailability,
};
use arp_scan_rs::scan_target::{apply_interface_to_input_state, prepare_scan_target, InputMode, UiInputState};
use arp_scan_rs::ui::{MainWindow, NetworkInterfaceItem, ResultListData};
```

At startup:

```rust
let interfaces = load_network_interfaces().unwrap_or_else(|_| Vec::new());
let interface_items = std::iter::once(NetworkInterfaceItem {
    label: "Select network interface".into(),
    enabled: true,
})
.chain(interfaces.iter().map(|item| NetworkInterfaceItem {
    label: format_interface_label(item).into(),
    enabled: matches!(item.availability, NetworkInterfaceAvailability::Available(_)),
}))
.collect::<Vec<_>>();

window.set_network_interface_model(ModelRc::from(std::rc::Rc::new(VecModel::from(interface_items))));
window.set_selected_network_interface_index(0);
```

Add a helper to map current window fields into `UiInputState`, then wire the selector callback:

```rust
let interfaces_for_selection = std::sync::Arc::new(interfaces);
let weak_window_for_interface = window.as_weak();
window.on_network_interface_changed(move |index| {
    if index <= 0 {
        return;
    }
    let Some(interface) = interfaces_for_selection.get((index - 1) as usize) else {
        return;
    };
    let Some(window) = weak_window_for_interface.upgrade() else {
        return;
    };

    let input_state = UiInputState {
        mode: if window.get_input_mode() == 1 {
            InputMode::Cidr
        } else {
            InputMode::IpAndMask
        },
        cidr_text: window.get_cidr_text().to_string(),
        ip_text: window.get_ip_text().to_string(),
        mask_text: window.get_mask_text().to_string(),
    };
    let next = apply_interface_to_input_state(&input_state, &interface.availability);

    window.set_cidr_text(next.cidr_text.into());
    window.set_ip_text(next.ip_text.into());
    window.set_mask_text(next.mask_text.into());
});
```

Implement real Windows enumeration in `src/network_interface.rs` by calling IP Helper APIs, and keep the non-Windows placeholder path from Task 1.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test ui_smoke -v`
Expected: PASS with the updated binding assertions

- [ ] **Step 5: Run focused regression tests**

Run: `cargo test --test network_interface --test scan_target --test ui_smoke -v`
Expected: PASS with all focused interface-selection tests green

- [ ] **Step 6: Run full verification**

Run:

```bash
cargo check
cargo test -v
```

Expected:

```text
Finished `dev` profile ... 
test result: ok. ... passed
```

- [ ] **Step 7: Commit**

```bash
git add src/network_interface.rs src/main.rs src/scan_target.rs src/lib.rs ui/main.slint tests/network_interface.rs tests/scan_target.rs tests/ui_smoke.rs
git commit -m "feat: 支持网络接口枚举与选择回填"
```
