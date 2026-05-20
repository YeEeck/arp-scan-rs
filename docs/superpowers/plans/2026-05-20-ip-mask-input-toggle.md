# IP And Mask Input Toggle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a user-facing switch between `CIDR` input and `IP + subnet mask` input while keeping scan startup internally normalized to a canonical CIDR string.

**Architecture:** The implementation keeps the scan runtime CIDR-only and adds a focused UI input normalization layer in Rust. Slint exposes explicit properties for mode and text fields, and `main.rs` converts the active UI input to CIDR before calling the existing scan startup path.

**Tech Stack:** Rust 2024, Slint 1.14, cargo test, existing scan runtime helpers

---

## File Structure

- Create: `src/scan_target.rs`
  - Own the input mode type and all `CIDR <-> IP + subnet mask` conversion and normalization helpers.
- Modify: `src/lib.rs`
  - Export the new `scan_target` module for tests and `main.rs`.
- Modify: `ui/main.slint`
  - Replace the single `cidr` binding with explicit mode/IP/mask/CIDR properties and a mode selector UI.
- Modify: `src/main.rs`
  - Initialize the new default input state and normalize active input to CIDR before starting a scan.
- Modify: `tests/ui_smoke.rs`
  - Cover the new generated bindings and default mode.
- Create: `tests/scan_target.rs`
  - Cover conversion, canonicalization, and invalid-input rejection.

### Task 1: Add failing scan-target normalization tests

**Files:**
- Create: `tests/scan_target.rs`
- Modify: `src/lib.rs`
- Test: `tests/scan_target.rs`

- [ ] **Step 1: Write the failing test**

```rust
use arp_scan_rs::scan_target::{normalize_scan_target, InputMode, ScanTargetInput};

#[test]
fn cidr_mode_preserves_valid_cidr() {
    let input = ScanTargetInput {
        mode: InputMode::Cidr,
        cidr_text: "192.168.1.0/24".into(),
        ip_text: String::new(),
        mask_text: String::new(),
    };

    assert_eq!(
        normalize_scan_target(&input).unwrap(),
        "192.168.1.0/24".to_string()
    );
}

#[test]
fn ip_and_mask_mode_normalizes_host_ip_to_network_cidr() {
    let input = ScanTargetInput {
        mode: InputMode::IpAndMask,
        cidr_text: String::new(),
        ip_text: "192.168.1.23".into(),
        mask_text: "255.255.255.0".into(),
    };

    assert_eq!(
        normalize_scan_target(&input).unwrap(),
        "192.168.1.0/24".to_string()
    );
}

#[test]
fn ip_and_mask_mode_rejects_non_contiguous_masks() {
    let input = ScanTargetInput {
        mode: InputMode::IpAndMask,
        cidr_text: String::new(),
        ip_text: "192.168.1.23".into(),
        mask_text: "255.0.255.0".into(),
    };

    assert_eq!(
        normalize_scan_target(&input).unwrap_err().to_string(),
        "Invalid subnet mask"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test scan_target -v`
Expected: FAIL with unresolved import errors for `arp_scan_rs::scan_target`

- [ ] **Step 3: Write minimal implementation**

```rust
pub mod scan_master;
pub mod scan_target;
pub mod ui;
pub mod ui_state;
```

```rust
use std::fmt;
use std::net::Ipv4Addr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    Cidr,
    IpAndMask,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanTargetInput {
    pub mode: InputMode,
    pub cidr_text: String,
    pub ip_text: String,
    pub mask_text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanTargetError(&'static str);

impl fmt::Display for ScanTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for ScanTargetError {}

pub fn normalize_scan_target(input: &ScanTargetInput) -> Result<String, ScanTargetError> {
    match input.mode {
        InputMode::Cidr => normalize_cidr(&input.cidr_text),
        InputMode::IpAndMask => normalize_ip_and_mask(&input.ip_text, &input.mask_text),
    }
}

fn normalize_cidr(cidr_text: &str) -> Result<String, ScanTargetError> {
    let (ip_text, prefix_text) = cidr_text
        .trim()
        .split_once('/')
        .ok_or(ScanTargetError("Invalid CIDR"))?;
    let ip: Ipv4Addr = ip_text.parse().map_err(|_| ScanTargetError("Invalid CIDR"))?;
    let prefix: u8 = prefix_text
        .parse()
        .map_err(|_| ScanTargetError("Invalid CIDR"))?;
    if prefix > 32 {
        return Err(ScanTargetError("Invalid CIDR"));
    }

    let mask = prefix_to_mask(prefix);
    let network = u32::from(ip) & mask;
    Ok(format!("{}/{}", Ipv4Addr::from(network), prefix))
}

fn normalize_ip_and_mask(ip_text: &str, mask_text: &str) -> Result<String, ScanTargetError> {
    let ip: Ipv4Addr = ip_text
        .trim()
        .parse()
        .map_err(|_| ScanTargetError("Invalid IP address"))?;
    let mask_ip: Ipv4Addr = mask_text
        .trim()
        .parse()
        .map_err(|_| ScanTargetError("Invalid subnet mask"))?;
    let mask = u32::from(mask_ip);
    let prefix = mask_to_prefix(mask).ok_or(ScanTargetError("Invalid subnet mask"))?;
    let network = u32::from(ip) & mask;

    Ok(format!("{}/{}", Ipv4Addr::from(network), prefix))
}

fn prefix_to_mask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        (!0u32) << (32 - prefix)
    }
}

fn mask_to_prefix(mask: u32) -> Option<u8> {
    let prefix = mask.leading_ones() as u8;
    if prefix_to_mask(prefix) == mask {
        Some(prefix)
    } else {
        None
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test scan_target -v`
Expected: PASS with `3 passed`

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/scan_target.rs tests/scan_target.rs
git commit -m "test: 添加扫描目标归一化覆盖"
```

### Task 2: Add failing mode-switch conversion tests

**Files:**
- Modify: `tests/scan_target.rs`
- Modify: `src/scan_target.rs`
- Test: `tests/scan_target.rs`

- [ ] **Step 1: Write the failing test**

```rust
use arp_scan_rs::scan_target::{
    convert_cidr_to_ip_and_mask, convert_ip_and_mask_to_cidr, InputMode, ScanTargetInput,
    UiInputState,
};

#[test]
fn cidr_converts_to_ip_and_mask_for_mode_switch() {
    assert_eq!(
        convert_cidr_to_ip_and_mask("192.168.1.0/24").unwrap(),
        ("192.168.1.0".to_string(), "255.255.255.0".to_string())
    );
}

#[test]
fn ip_and_mask_convert_to_cidr_for_mode_switch() {
    assert_eq!(
        convert_ip_and_mask_to_cidr("192.168.1.23", "255.255.255.128").unwrap(),
        "192.168.1.0/25".to_string()
    );
}

#[test]
fn invalid_mode_switch_keeps_last_known_destination_values() {
    let state = UiInputState {
        mode: InputMode::Cidr,
        cidr_text: "not-a-cidr".into(),
        ip_text: "10.0.0.8".into(),
        mask_text: "255.255.255.0".into(),
    };

    let switched = state.switch_mode(InputMode::IpAndMask);

    assert_eq!(switched.mode, InputMode::IpAndMask);
    assert_eq!(switched.ip_text, "10.0.0.8");
    assert_eq!(switched.mask_text, "255.255.255.0");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test scan_target -v`
Expected: FAIL with unresolved imports for the new conversion helpers or `UiInputState`

- [ ] **Step 3: Write minimal implementation**

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiInputState {
    pub mode: InputMode,
    pub cidr_text: String,
    pub ip_text: String,
    pub mask_text: String,
}

impl UiInputState {
    pub fn switch_mode(&self, next_mode: InputMode) -> Self {
        if self.mode == next_mode {
            return self.clone();
        }

        let mut next = self.clone();
        next.mode = next_mode;

        match (self.mode, next_mode) {
            (InputMode::Cidr, InputMode::IpAndMask) => {
                if let Ok((ip_text, mask_text)) = convert_cidr_to_ip_and_mask(&self.cidr_text) {
                    next.ip_text = ip_text;
                    next.mask_text = mask_text;
                }
            }
            (InputMode::IpAndMask, InputMode::Cidr) => {
                if let Ok(cidr_text) = convert_ip_and_mask_to_cidr(&self.ip_text, &self.mask_text)
                {
                    next.cidr_text = cidr_text;
                }
            }
            _ => {}
        }

        next
    }
}

pub fn convert_cidr_to_ip_and_mask(cidr_text: &str) -> Result<(String, String), ScanTargetError> {
    let cidr = normalize_cidr(cidr_text)?;
    let (ip_text, prefix_text) = cidr
        .split_once('/')
        .ok_or(ScanTargetError("Invalid CIDR"))?;
    let prefix: u8 = prefix_text
        .parse()
        .map_err(|_| ScanTargetError("Invalid CIDR"))?;

    Ok((
        ip_text.to_string(),
        Ipv4Addr::from(prefix_to_mask(prefix)).to_string(),
    ))
}

pub fn convert_ip_and_mask_to_cidr(
    ip_text: &str,
    mask_text: &str,
) -> Result<String, ScanTargetError> {
    normalize_ip_and_mask(ip_text, mask_text)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test scan_target -v`
Expected: PASS with `6 passed`

- [ ] **Step 5: Commit**

```bash
git add src/scan_target.rs tests/scan_target.rs
git commit -m "test: 添加输入模式切换转换覆盖"
```

### Task 3: Add failing UI binding test for the new properties

**Files:**
- Modify: `tests/ui_smoke.rs`
- Modify: `ui/main.slint`
- Test: `tests/ui_smoke.rs`

- [ ] **Step 1: Write the failing test**

```rust
use arp_scan_rs::ui::{MainWindow, ResultListData};
use slint::{Model, ModelRc, VecModel};

#[test]
fn main_window_smoke_test_exposes_generated_bindings() {
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

    window.set_result_list_data_model(ModelRc::from(std::rc::Rc::new(rows)));
    window.set_input_mode(0);
    window.set_cidr_text("192.168.1.0/24".into());
    window.set_ip_text("192.168.1.23".into());
    window.set_mask_text("255.255.255.0".into());
    window.set_status_text("Scanning".into());
    window.set_progress_text("2 / 254".into());
    window.set_scan_enabled(false);
    window.set_cancel_enabled(true);

    assert_eq!(window.get_input_mode(), 0);
    assert_eq!(window.get_cidr_text(), "192.168.1.0/24");
    assert_eq!(window.get_ip_text(), "192.168.1.23");
    assert_eq!(window.get_mask_text(), "255.255.255.0");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test ui_smoke -v`
Expected: FAIL with missing generated binding methods for `input_mode`, `cidr_text`, `ip_text`, or `mask_text`

- [ ] **Step 3: Write minimal implementation**

```slint
import { Button, ComboBox, GroupBox, LineEdit, ListView } from "std-widgets.slint";
```

```slint
in-out property <int> input_mode: 0;
in-out property <string> cidr_text;
in-out property <string> ip_text: "192.168.1.23";
in-out property <string> mask_text: "255.255.255.0";
```

```slint
GroupBox {
    title: "Scan Target";
    vertical-stretch: 0;

    VerticalLayout {
        spacing: root.control-row-spacing;

        ComboBox {
            min-height: root.control-min-height;
            current-index <=> root.input_mode;
            model: ["IP + Subnet Mask", "CIDR"];
            enabled: root.scan_enabled;
        }

        if root.input_mode == 0: HorizontalLayout {
            spacing: root.control-row-spacing;

            LineEdit {
                min-height: root.control-min-height;
                placeholder-text: "192.168.1.23";
                text <=> root.ip_text;
                enabled: root.scan_enabled;
                font-size: root.control_font_size;
            }

            LineEdit {
                min-height: root.control-min-height;
                placeholder-text: "255.255.255.0";
                text <=> root.mask_text;
                enabled: root.scan_enabled;
                font-size: root.control_font_size;
            }
        }

        if root.input_mode == 1: LineEdit {
            min-height: root.control-min-height;
            placeholder-text: "192.168.1.0/24";
            text <=> root.cidr_text;
            enabled: root.scan_enabled;
            font-size: root.control_font_size;
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test ui_smoke -v`
Expected: PASS with `1 passed`

- [ ] **Step 5: Commit**

```bash
git add ui/main.slint tests/ui_smoke.rs
git commit -m "test: 覆盖扫描目标输入切换绑定"
```

### Task 4: Wire scan startup through the normalization layer

**Files:**
- Modify: `src/main.rs`
- Modify: `src/scan_target.rs`
- Test: `tests/scan_target.rs`

- [ ] **Step 1: Write the failing test**

```rust
use arp_scan_rs::scan_target::{prepare_scan_target, InputMode, UiInputState};

#[test]
fn prepare_scan_target_uses_visible_ip_and_mask_mode() {
    let state = UiInputState {
        mode: InputMode::IpAndMask,
        cidr_text: "10.0.0.0/8".into(),
        ip_text: "192.168.1.23".into(),
        mask_text: "255.255.255.0".into(),
    };

    assert_eq!(prepare_scan_target(&state).unwrap(), "192.168.1.0/24");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test scan_target -v`
Expected: FAIL with unresolved import for `prepare_scan_target`

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn prepare_scan_target(state: &UiInputState) -> Result<String, ScanTargetError> {
    normalize_scan_target(&ScanTargetInput {
        mode: state.mode,
        cidr_text: state.cidr_text.clone(),
        ip_text: state.ip_text.clone(),
        mask_text: state.mask_text.clone(),
    })
}
```

```rust
use arp_scan_rs::scan_target::{prepare_scan_target, InputMode, UiInputState};
```

```rust
let window = MainWindow::new().unwrap();
window.set_input_mode(0);
window.set_ip_text("192.168.1.23".into());
window.set_mask_text("255.255.255.0".into());
window.set_cidr_text("192.168.1.0/24".into());
```

```rust
let input_mode = match window.get_input_mode() {
    0 => InputMode::IpAndMask,
    _ => InputMode::Cidr,
};
let cidr = match prepare_scan_target(&UiInputState {
    mode: input_mode,
    cidr_text: window.get_cidr_text().to_string(),
    ip_text: window.get_ip_text().to_string(),
    mask_text: window.get_mask_text().to_string(),
}) {
    Ok(cidr) => cidr,
    Err(err) => {
        let mut state = ui_state_for_start.lock().unwrap();
        let row_mutation = state.fail_to_start(err.to_string());
        let view_state = state.view_state();
        drop(state);
        apply_view_state(&window, view_state);
        apply_row_mutation(&window, row_mutation);
        return;
    }
};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test scan_target -v`
Expected: PASS with `7 passed`

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/scan_target.rs tests/scan_target.rs
git commit -m "feat: 接入扫描目标输入归一化"
```

### Task 5: Verify full workspace behavior

**Files:**
- Modify: `docs/superpowers/plans/2026-05-20-ip-mask-input-toggle.md`

- [x] **Step 1: Run focused tests**

Run: `cargo test --test scan_target --test ui_smoke -v`
Expected: PASS with all targeted tests green

- [x] **Step 2: Run full verification**

Run: `cargo check && cargo test -v`
Expected: `cargo check` exits 0 and `cargo test -v` exits 0 with no failing tests

- [x] **Step 3: Mark plan progress**

```markdown
- [x] Task 1 completed
- [x] Task 2 completed
- [x] Task 3 completed
- [x] Task 4 completed
- [x] Task 5 completed
```

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/plans/2026-05-20-ip-mask-input-toggle.md
git commit -m "docs: 记录 IP 与子网掩码输入切换实现计划"
```
