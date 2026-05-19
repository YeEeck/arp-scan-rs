# Window Resize And Windows Typography Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Slint desktop window freely resizable, keep the result list confined to its own scrollable viewport, and improve Windows desktop readability with clearer typography and spacing.

**Architecture:** Keep the implementation centered in `ui/main.slint`. The top form area will use fixed-height desktop sizing tokens, while the result pane becomes the single flexible region using Slint stretch, clipping, and text elision. No scan runtime or Rust state-flow changes are planned.

**Tech Stack:** Rust 2024, Slint 1.14, std-widgets.slint, Cargo

---

## File Structure

- Modify: `ui/main.slint`
  - Own the full window layout, sizing tokens, typography hierarchy, column widths, clipping, and resize behavior.
- Verify: `build.rs`
  - No code change expected; `slint_build::compile("ui/main.slint")` is the compile gate exercised by `cargo check` and `cargo test`.
- Regression coverage: existing cargo test suite
  - `tests/ui_state.rs`
  - `tests/scan_runtime.rs`
  - `tests/ip_range.rs`
  - `tests/probe_stub.rs`
  - `tests/windows_link_name.rs`

### Task 1: Resizable Window Shell And Desktop Tokens

**Files:**
- Modify: `ui/main.slint`
- Verify: `build.rs`
- Test: `cargo check`
- Test: `cargo test -v`

- [ ] **Step 1: Replace the window shell with explicit desktop sizing and typography tokens**

Update the `MainWindow` header and top-level token definitions in `ui/main.slint` to this shape:

```slint
export component MainWindow inherits Window {
    title: "ArpScan-rs";
    width: 720px;
    height: 640px;
    min-width: 560px;
    min-height: 520px;
    default-font-family: "Segoe UI";
    default-font-size: 15px;

    private property <length> page-padding: 24px;
    private property <length> section-spacing: 14px;
    private property <length> control-row-spacing: 10px;
    private property <length> control-min-height: 42px;
    private property <length> status-font-size: 15px;
    private property <length> header-font-size: 15px;
    private property <length> row-font-size: 15px;
    private property <length> result-header-height: 34px;
    private property <length> result-row-height: 34px;
    private property <length> result-panel-padding: 10px;
    private property <length> result-column-spacing: 14px;
    private property <length> ip-column-width: 190px;
    private property <length> mac-column-min-width: 240px;

    in property <[ResultListData]> result_list_data_model;
    in-out property <string> cidr;
    in property <string> status_text;
    in property <string> progress_text;
    in property <bool> scan_enabled;
    in property <bool> cancel_enabled;

    callback do_scan();
    callback do_cancel();
```

This keeps the UI on a predictable Windows-friendly scale and establishes the min-size contract for free resizing.

- [ ] **Step 2: Rebuild the top form layout so it stretches horizontally but keeps stable vertical sizing**

Replace the current top layout block in `ui/main.slint` with this structure:

```slint
    VerticalLayout {
        padding: root.page-padding;
        spacing: root.section-spacing;

        GroupBox {
            title: "CIDR";

            LineEdit {
                min-height: root.control-min-height;
                text <=> root.cidr;
                placeholder-text: "192.168.1.0/24";
                enabled: root.scan_enabled;
                font-size: root.row-font-size;
            }
        }

        HorizontalLayout {
            spacing: root.control-row-spacing;

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
        }

        Text {
            text: root.progress_text;
            font-size: root.status-font-size;
        }
```

This ensures the CIDR field, buttons, status line, and progress line grow with width but do not consume the result pane’s vertical budget.

- [ ] **Step 3: Run compile verification for the resizable shell**

Run:

```bash
cargo check
```

Expected:

```text
Finished `dev` profile ...
```

The exact timing will vary, but `cargo check` must succeed with no Slint compile errors.

- [ ] **Step 4: Run the regression suite after the shell change**

Run:

```bash
cargo test -v
```

Expected:

```text
test result: ok.
```

All existing tests must still pass because this task changes only UI layout code.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add ui/main.slint
git commit -m "feat: add resizable desktop window shell"
```

### Task 2: Confined Result Pane, Single-Line Columns, And Final Desktop Polish

**Files:**
- Modify: `ui/main.slint`
- Verify: `build.rs`
- Test: `cargo check`
- Test: `cargo test -v`

- [ ] **Step 1: Replace the fixed-height result container with a stretching, clipped panel**

Replace the current result area in `ui/main.slint` with this structure:

```slint
        Rectangle {
            min-height: 0px;
            horizontal-stretch: 1;
            vertical-stretch: 1;
            clip: true;
            border-width: 1px;
            border-color: #d0d7de;
            background: #ffffff;

            VerticalLayout {
                padding: root.result-panel-padding;
                spacing: 8px;

                Rectangle {
                    clip: true;
                    vertical-stretch: 0;
                    min-height: root.result-header-height;
                    background: #f6f8fa;

                    header-layout := HorizontalLayout {
                        spacing: root.result-column-spacing;
                        min-height: root.result-header-height;

                        Rectangle {
                            min-width: root.ip-column-width;
                            preferred-width: root.ip-column-width;
                            max-width: root.ip-column-width;

                            Text {
                                width: 100%;
                                height: 100%;
                                text: "IP";
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
                                width: 100%;
                                height: 100%;
                                text: "MAC";
                                font-size: root.header-font-size;
                                font-weight: 700;
                                vertical-alignment: center;
                                overflow: elide;
                            }
                        }
                    }
                }
```

This is the key overflow fix: the result panel becomes the only vertical-stretch region and clips its own descendants.

- [ ] **Step 2: Rebuild the list rows to stay single-line and elide inside the panel**

Continue the result panel replacement with this `ListView` block:

```slint
                ListView {
                    min-height: 0px;
                    horizontal-stretch: 1;
                    vertical-stretch: 1;

                    for row in root.result_list_data_model: Rectangle {
                        min-height: root.result-row-height;
                        clip: true;

                        HorizontalLayout {
                            spacing: root.result-column-spacing;

                            Rectangle {
                                min-width: root.ip-column-width;
                                preferred-width: root.ip-column-width;
                                max-width: root.ip-column-width;
                                clip: true;

                                Text {
                                    width: 100%;
                                    height: 100%;
                                    text: row.ip;
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
                                    width: 100%;
                                    height: 100%;
                                    text: row.mac;
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
    }
}
```

This preserves single-line readability for both columns. In narrow windows, the text elides instead of wrapping or escaping the viewport.

- [ ] **Step 3: Run compile verification for the final UI**

Run:

```bash
cargo check
```

Expected:

```text
Finished `dev` profile ...
```

The Slint build must succeed after the result-pane replacement.

- [ ] **Step 4: Run the full regression suite and do manual acceptance checks**

Run:

```bash
cargo test -v
```

Expected:

```text
test result: ok.
```

Then manually verify:

```text
1. The window can be dragged larger and smaller.
2. The result list stays inside its bordered panel.
3. Extra height mostly expands the result list.
4. Narrow widths keep IP and MAC single-line with clipped text.
5. Windows 1080p rendering looks less cramped than the original UI.
```

- [ ] **Step 5: Commit Task 2**

Run:

```bash
git add ui/main.slint
git commit -m "fix: constrain result list in resizable layout"
```
