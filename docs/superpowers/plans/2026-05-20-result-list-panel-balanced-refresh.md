# Result List Panel Balanced Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refresh the Slint result list panel into a cleaner balanced table that improves hierarchy and hostname readability without changing scan behavior.

**Architecture:** Keep all scan/runtime and Rust-side UI-state plumbing untouched. Implement the refresh entirely in `ui/main.slint`, preserving the existing three-column data model, the scroll-synced header layout, and the stable hostname test hooks that current smoke coverage depends on. Use the existing UI smoke tests plus the hostname-binding regression test as the safety net while updating only presentation constants and row/header styling.

**Tech Stack:** Rust 2024, Slint 1.14, std-widgets.slint, Cargo, i-slint-backend-testing

---

## File Structure

- Modify: `ui/main.slint`
  - Refine result panel constants, header styling, row styling, and column balance while preserving `IP | MAC | Hostname` and the stable hostname test ids.
- Modify: `tests/result_list_panel_smoke.rs`
  - Extend the existing smoke coverage only if needed to lock in any styling-adjacent structure assumptions introduced by the refresh.
- Modify: `tests/ui_smoke.rs`
  - Touch only if the refresh requires a narrow assertion update for the main window wiring.

### Task 1: Refresh The Result List Panel Styling In Slint

**Files:**
- Modify: `ui/main.slint`
- Modify: `tests/result_list_panel_smoke.rs` (only if required)
- Modify: `tests/ui_smoke.rs` (only if required)

- [ ] **Step 1: Add a focused failing test for the balanced refresh guardrail**

Add this test to `tests/result_list_panel_smoke.rs` below the existing tests:

```rust
#[test]
fn result_list_panel_keeps_hostname_column_reachable_with_empty_and_filled_rows() {
    let _backend = i_slint_backend_testing::init_no_event_loop();

    let panel = ResultListPanel::new().unwrap();
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

    panel.set_rows(ModelRc::from(std::rc::Rc::new(rows)));

    let hostname_header = ElementHandle::find_by_element_id(
        &panel,
        "ResultListPanel::hostname-column-header",
    )
    .next()
    .expect("hostname column header should stay present");
    assert_eq!(hostname_header.accessible_value().as_deref(), Some("Hostname"));

    let hostname_cell =
        ElementHandle::find_by_element_id(&panel, "ResultListPanel::hostname-cell-text")
            .next()
            .expect("hostname cell text should stay present");
    assert_eq!(
        hostname_cell.accessible_value().as_deref(),
        Some("printer.local")
    );
}
```

This is intentionally redundant with the current regression direction: it gives the styling task a narrow red/green guard that fails if the refresh accidentally drops the hostname column hook or its binding while rearranging the panel.

- [ ] **Step 2: Run the focused test to verify it fails for the right reason**

Run:

```bash
cargo test --test result_list_panel_smoke result_list_panel_keeps_hostname_column_reachable_with_empty_and_filled_rows -- --nocapture
```

Expected:

```text
running 1 test
test result_list_panel_keeps_hostname_column_reachable_with_empty_and_filled_rows ... FAILED
```

The failure should be the usual “test not found” or missing assertion failure caused by the new test not existing yet. Do not proceed until you have observed the red state.

- [ ] **Step 3: Write the minimal styling implementation**

Update `ui/main.slint` so the result panel follows the balanced-refresh spec while preserving all current behavior:

```slint
component ResultListPanel inherits Rectangle {
    in property <[ResultListData]> rows;

    private property <length> panel-padding: 12px;
    private property <length> panel-inset: 1px;
    private property <length> header-height: 36px;
    private property <length> row-height: 38px;
    private property <length> column-spacing: 10px;
    private property <length> ip-column-width: 168px;
    private property <length> mac-column-width: 164px;
    private property <length> hostname-column-min-width: 96px;
    private property <length> row-font-size: 15px;
    private property <length> header-font-size: 14px;
```

Adjust the outer panel treatment to a slightly clearer tool-panel surface:

```slint
    clip: true;
    border-width: 1px;
    border-color: #c4ccd7;
    border-radius: 12px;
    background: #f7f9fc;
```

Refine the header band:

```slint
        Rectangle {
            clip: true;
            vertical-stretch: 0;
            height: root.header-height;
            border-radius: 8px;
            background: #e7edf5;

            Rectangle {
                y: parent.height - 1px;
                width: parent.width;
                height: 1px;
                background: #cfd8e3;
            }
```

Keep the current `hostname-column-header := Text` id and binding, but align all three header labels to the same visual inset and slightly soften the color hierarchy:

```slint
                        color: #314050;
                        font-size: root.header-font-size;
                        font-weight: 700;
```

Refresh the row presentation while preserving the same data bindings:

```slint
            for [index] row in root.rows: Rectangle {
                min-height: root.row-height;
                horizontal-stretch: 1;
                clip: true;
                background: index % 2 == 0 ? #fcfdff : #f7f9fc;
```

Inside the row body, keep the existing `hostname-cell-text := Text` id and `text: row.hostname` binding, but make the row surface and separator lighter and more deliberate:

```slint
                    Rectangle {
                        min-height: root.row-height - 1px;
                        horizontal-stretch: 1;
                        background: index % 2 == 0 ? #fcfdff : #f7f9fc;
```

```slint
                    Rectangle {
                        height: 1px;
                        horizontal-stretch: 1;
                        vertical-stretch: 0;
                        background: #dbe3ec;
                    }
```

While making these updates:

- preserve column order `IP | MAC | Hostname`
- preserve the scroll-linked header position with `x: result-list.viewport-x`
- preserve the stable ids:
  - `hostname-column-header`
  - `hostname-cell-text`
- preserve:
  - `text: "Hostname"` for the header
  - `text: row.hostname` for the hostname cell
- do not change `ResultListData`
- do not change any Rust files for this task unless a test adjustment requires it

- [ ] **Step 4: Run focused verification**

Run:

```bash
cargo test --test result_list_panel_smoke --test ui_smoke -v
```

Expected:

```text
test result: ok.
```

The existing hostname-binding regression and smoke coverage must stay green.

- [ ] **Step 5: Run integration verification**

Run:

```bash
cargo check
```

Expected:

```text
Finished `dev` profile ...
```

The existing Slint warning about exporting `ResultListPanel` may still appear and is non-blocking for this task.

- [ ] **Step 6: Commit**

Run:

```bash
git add ui/main.slint tests/result_list_panel_smoke.rs tests/ui_smoke.rs
git commit -m "style: refresh balanced result list panel"
```

If `tests/ui_smoke.rs` is untouched, omit it from `git add` rather than forcing a no-op edit.
