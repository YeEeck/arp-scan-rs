# Result List Panel Styling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refresh the Slint scan result list into a balanced-density tool panel with a clearer header, cleaner container styling, and easier-to-scan rows without changing scan behavior.

**Architecture:** Keep scan logic and Rust-side state flow untouched. Move the generated Slint types into a library-owned `ui` module so integration tests can instantiate them, then extract the result panel into an exported Slint component inside `ui/main.slint` and wire `MainWindow` to use that styled component.

**Tech Stack:** Rust 2024, Slint 1.14, std-widgets.slint, Cargo

---

## File Structure

- Modify: `Cargo.toml`
  - Add the Slint testing backend as a dev-dependency for headless UI smoke tests.
- Modify: `src/lib.rs`
  - Export a new library-owned `ui` module alongside the existing scan and UI-state modules.
- Create: `src/ui.rs`
  - Own the generated Slint Rust bindings via `slint::include_modules!()`.
- Modify: `src/main.rs`
  - Import `MainWindow` and `ResultListData` from the library `ui` module instead of generating types in the binary.
- Modify: `ui/main.slint`
  - Introduce the exported `ResultListPanel` component and move the result-list styling there.
- Create: `tests/ui_smoke.rs`
  - Verify `MainWindow` can be instantiated headlessly and still accepts sample properties and rows after the library UI-module move.
- Create: `tests/result_list_panel_smoke.rs`
  - Verify the new exported `ResultListPanel` component exists and accepts a sample model after the panel extraction.

### Task 1: Expose The Generated Slint UI Through The Library And Add A Main Window Smoke Test

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`
- Create: `src/ui.rs`
- Modify: `src/main.rs`
- Create: `tests/ui_smoke.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/ui_smoke.rs` with this content:

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
    window.set_cidr("192.168.1.0/24".into());
    window.set_status_text("Finished (2 hosts)".into());
    window.set_progress_text("254 / 254".into());
    window.set_scan_enabled(true);
    window.set_cancel_enabled(false);

    assert_eq!(window.get_cidr(), "192.168.1.0/24");
    assert_eq!(window.get_status_text(), "Finished (2 hosts)");
    assert_eq!(window.get_progress_text(), "254 / 254");
    assert!(window.get_scan_enabled());
    assert!(!window.get_cancel_enabled());

    let model_rc = window.get_result_list_data_model();
    let model = model_rc
        .as_any()
        .downcast_ref::<VecModel<ResultListData>>()
        .expect("result model should remain a VecModel<ResultListData>");

    assert_eq!(model.row_count(), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test ui_smoke -v
```

Expected:

```text
error[E0432]: unresolved import `arp_scan_rs::ui`
```

The exact line numbers may differ, but the test must fail because the crate does not yet export the generated Slint UI types.

- [ ] **Step 3: Write minimal implementation**

Update the crate to own the generated Slint bindings in the library.

`Cargo.toml`

```toml
[package]
name = "arp-scan-rs"
version = "0.1.0"
edition = "2024"

[dependencies]
clap = { version = "4.5.53", features = ["derive"] }
slint = "1.14.1"

[dev-dependencies]
i-slint-backend-testing = "1.14.1"

[build-dependencies]
slint-build = "1.14.1"

[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.52", features = ["Win32_NetworkManagement_IpHelper"] }
```

`src/lib.rs`

```rust
pub mod scan_master;
pub mod ui;
pub mod ui_state;
```

`src/ui.rs`

```rust
slint::include_modules!();
```

At the top of `src/main.rs`, replace the generated-type setup with imports from the library:

```rust
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use arp_scan_rs::scan_master::{start_scan, ScanEvent, ScanTask};
use arp_scan_rs::ui::{MainWindow, ResultListData};
use arp_scan_rs::ui_state::{RowMutation, ScanUiState, ViewState};
use slint::{Model, ModelRc, VecModel};
```

Remove this line from `src/main.rs`:

```rust
slint::include_modules!();
```

No other behavior changes are needed in this task.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test ui_smoke -v
```

Expected:

```text
test main_window_accepts_sample_result_rows ... ok
test result: ok.
```

- [ ] **Step 5: Commit**

Run:

```bash
git add Cargo.toml src/lib.rs src/main.rs src/ui.rs tests/ui_smoke.rs
git commit -m "test: expose slint ui for smoke coverage"
```

### Task 2: Extract And Style The Result List Panel As A Tool-Oriented Slint Component

**Files:**
- Modify: `ui/main.slint`
- Create: `tests/result_list_panel_smoke.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/result_list_panel_smoke.rs` with this content:

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

    let model_rc = panel.get_rows();
    let model = model_rc
        .as_any()
        .downcast_ref::<VecModel<ResultListData>>()
        .expect("rows should remain a VecModel<ResultListData>");

    assert_eq!(model.row_count(), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test result_list_panel_smoke -v
```

Expected:

```text
error[E0432]: unresolved import `arp_scan_rs::ui::ResultListPanel`
```

The failure confirms that the dedicated panel component does not exist yet.

- [ ] **Step 3: Write minimal implementation**

Replace `ui/main.slint` with a version that exports a dedicated `ResultListPanel` and rewires `MainWindow` to use it.

```slint
import { Button, GroupBox, LineEdit, ListView } from "std-widgets.slint";

export struct ResultListData {
    ip: string,
    mac: string,
}

export component ResultListPanel inherits Rectangle {
    in property <[ResultListData]> rows;

    private property <length> panel-padding: 10px;
    private property <length> panel-radius: 12px;
    private property <length> header-height: 34px;
    private property <length> row-height: 34px;
    private property <length> column-spacing: 14px;
    private property <length> cell-horizontal-padding: 14px;
    private property <length> ip-column-width: 190px;
    private property <length> mac-column-min-width: 0px;
    private property <length> header-font-size: 15px;
    private property <length> row-font-size: 15px;
    private property <brush> panel-border: #cfd8e3;
    private property <brush> panel-background: #ffffff;
    private property <brush> header-background: #e9eff6;
    private property <brush> header-border: #d8e1ec;
    private property <brush> header-text-color: #1f2f3d;
    private property <brush> row-divider: #e7edf4;
    private property <brush> row-even-background: #f8fbff;
    private property <brush> row-odd-background: #ffffff;
    private property <brush> row-text-color: #243447;

    min-height: 0px;
    horizontal-stretch: 1;
    vertical-stretch: 1;
    clip: true;
    border-radius: root.panel-radius;
    border-width: 1px;
    border-color: root.panel-border;
    background: root.panel-background;

    VerticalLayout {
        padding: root.panel-padding;
        spacing: 8px;

        Rectangle {
            clip: true;
            vertical-stretch: 0;
            height: root.header-height;
            border-radius: 8px;
            border-width: 1px;
            border-color: root.header-border;
            background: root.header-background;

            header-layout := HorizontalLayout {
                height: root.header-height;
                width: max(self.preferred-width, result-list.visible-width);
                x: result-list.viewport-x;
                spacing: root.column-spacing;

                Rectangle {
                    min-width: root.ip-column-width;
                    preferred-width: root.ip-column-width;
                    max-width: root.ip-column-width;

                    Text {
                        x: root.cell-horizontal-padding;
                        width: parent.width - 2 * root.cell-horizontal-padding;
                        height: 100%;
                        text: "IP";
                        color: root.header-text-color;
                        font-size: root.header-font-size;
                        font-weight: 700;
                        vertical-alignment: center;
                        overflow: elide;
                    }
                }

                Rectangle {
                    min-width: root.mac-column-min-width;
                    horizontal-stretch: 1;

                    Text {
                        x: root.cell-horizontal-padding;
                        width: parent.width - 2 * root.cell-horizontal-padding;
                        height: 100%;
                        text: "MAC";
                        color: root.header-text-color;
                        font-size: root.header-font-size;
                        font-weight: 700;
                        vertical-alignment: center;
                        overflow: elide;
                    }
                }
            }
        }

        result-list := ListView {
            min-height: 0px;
            horizontal-stretch: 1;
            vertical-stretch: 1;

            for row[index] in root.rows: Rectangle {
                min-height: root.row-height;
                horizontal-stretch: 1;
                clip: true;
                background: mod(index, 2) == 0 ? root.row-even-background : root.row-odd-background;

                Rectangle {
                    x: 0;
                    y: self.parent.height - 1px;
                    width: self.parent.width;
                    height: 1px;
                    background: root.row-divider;
                }

                HorizontalLayout {
                    spacing: root.column-spacing;

                    Rectangle {
                        min-width: root.ip-column-width;
                        preferred-width: root.ip-column-width;
                        max-width: root.ip-column-width;
                        clip: true;

                        Text {
                            x: root.cell-horizontal-padding;
                            width: parent.width - 2 * root.cell-horizontal-padding;
                            height: 100%;
                            text: row.ip;
                            color: root.row-text-color;
                            font-size: root.row-font-size;
                            vertical-alignment: center;
                            overflow: elide;
                        }
                    }

                    Rectangle {
                        min-width: root.mac-column-min-width;
                        horizontal-stretch: 1;
                        clip: true;

                        Text {
                            x: root.cell-horizontal-padding;
                            width: parent.width - 2 * root.cell-horizontal-padding;
                            height: 100%;
                            text: row.mac;
                            color: root.row-text-color;
                            font-size: root.row-font-size;
                            vertical-alignment: center;
                            overflow: elide;
                        }
                    }
                }
            }
        }
    }
}

export component MainWindow inherits Window {
    title: "ArpScan-rs";
    preferred-width: 720px;
    preferred-height: 640px;
    min-width: 560px;
    min-height: 520px;
    default-font-family: "Segoe UI";
    default-font-size: 15px;

    private property <length> page-padding: 24px;
    private property <length> section-spacing: 14px;
    private property <length> control-row-spacing: 10px;
    private property <length> control-min-height: 42px;
    private property <length> status-font-size: 15px;
    private property <length> row-font-size: 15px;

    in property <[ResultListData]> result_list_data_model;
    in-out property <string> cidr;
    in property <string> status_text;
    in property <string> progress_text;
    in property <bool> scan_enabled;
    in property <bool> cancel_enabled;

    callback do_scan();
    callback do_cancel();

    VerticalLayout {
        padding: root.page-padding;
        spacing: root.section-spacing;

        GroupBox {
            title: "CIDR";
            vertical-stretch: 0;

            LineEdit {
                min-height: root.control-min-height;
                placeholder-text: "192.168.1.0/24";
                text <=> root.cidr;
                enabled: root.scan_enabled;
                font-size: root.row-font-size;
            }
        }

        HorizontalLayout {
            spacing: root.control-row-spacing;
            vertical-stretch: 0;

            Button {
                min-height: root.control-min-height;
                text: "Scan";
                enabled: root.scan_enabled;
                clicked => {
                    root.do_scan();
                }
            }

            Button {
                min-height: root.control-min-height;
                text: "Cancel";
                enabled: root.cancel_enabled;
                clicked => {
                    root.do_cancel();
                }
            }
        }

        Text {
            text: root.status_text;
            font-size: root.status-font-size;
            font-weight: 600;
            vertical-stretch: 0;
        }

        Text {
            text: root.progress_text;
            font-size: root.status-font-size;
            vertical-stretch: 0;
        }

        ResultListPanel {
            rows: root.result_list_data_model;
        }
    }
}
```

This implementation keeps the current data flow intact, gives the result area an explicit tool-panel surface, strengthens the header, and adds light zebra striping plus separators without switching to card rows.

- [ ] **Step 4: Run the targeted UI smoke tests**

Run:

```bash
cargo test --test ui_smoke --test result_list_panel_smoke -v
```

Expected:

```text
test main_window_accepts_sample_result_rows ... ok
test result_list_panel_accepts_sample_rows ... ok
test result: ok.
```

- [ ] **Step 5: Run compile verification**

Run:

```bash
cargo check
```

Expected:

```text
Finished `dev` profile ...
```

The exact timing may vary, but `cargo check` must succeed with no Slint compile errors.

- [ ] **Step 6: Run the full regression suite**

Run:

```bash
cargo test -v
```

Expected:

```text
test result: ok.
```

All existing tests must still pass because the Rust scan logic and UI-state reducer are unchanged.

- [ ] **Step 7: Commit**

Run:

```bash
git add ui/main.slint tests/result_list_panel_smoke.rs
git commit -m "feat: refresh result list panel styling"
```
