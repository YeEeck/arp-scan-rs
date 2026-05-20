# Result List Panel Balanced Refresh Design

## Summary

This design applies a second-pass visual refresh to the scan result list panel
now that the panel has three columns:

- `IP`
- `MAC`
- `Hostname`

The selected direction is a balanced utility-table style:

- cleaner and more intentional than the current panel
- still restrained and tool-like
- improved vertical scanning for mixed empty and resolved hostname rows

The scan workflow, result ordering, reverse-DNS behavior, and Rust-side state
flow remain unchanged.

## Current Problems

- The result panel is functional but still looks visually flat after the
  hostname column addition.
- The current header and row treatments do not create enough hierarchy between
  structure and data.
- `Hostname` now competes with a fixed-width `MAC` column, so the panel feels
  less balanced on tighter window widths.
- Rows are readable, but the list still lacks enough rhythm to support fast
  scanning across longer result sets.

## Goals

- Keep the result panel recognizably table-like and utility-focused.
- Improve hierarchy between outer panel, header, rows, and dividers.
- Make `IP | MAC | Hostname` feel visually balanced rather than appended.
- Preserve practical information density.
- Improve `Hostname` readability without making `IP` and `MAC` alignment feel
  loose.
- Keep blank hostname cells visually calm and unobtrusive.

## Non-Goals

- Changing scan behavior, result ordering, or reverse-DNS timing.
- Adding row selection, copy affordances, sorting, filtering, or context menus.
- Replacing the current custom header plus `ListView` structure.
- Introducing card-style rows or a dashboard-like visual language.
- Restyling the entire window beyond what is needed to keep the result area
  coherent.

## Chosen Visual Direction

### Selected style

The approved direction is a balanced table refresh:

- soft but still structured container
- clearer header band
- subtle row rhythm through alternating surfaces and dividers
- conservative hover feedback if it is low-risk in the current Slint layout

### Why this direction

- It matches the app’s purpose as a compact desktop network utility.
- It improves readability without turning the panel into a decorative widget.
- It fits the existing Slint structure with low implementation risk.

## Visual Design Rules

### Container

- Keep a single result container.
- Strengthen the panel edge slightly compared with the current flat border.
- Use moderate corner radius and a very light elevated feel.
- Keep the main reading surface bright and neutral.

### Header

- Keep the header aligned to horizontal scrolling exactly as today.
- Increase distinction between header and rows through background, contrast, and
  separator treatment.
- Keep header labels concise, single-line, and clearly column-aligned.

### Rows

- Preserve simple table rows, not cards.
- Use a slightly more deliberate row height and horizontal padding.
- Add subtle zebra striping and cleaner row separators.
- If hover can be added cleanly without fighting `ListView`, keep it subdued.
- Blank hostname cells should remain blank rather than showing placeholders.

### Column Balance

- Preserve the approved order: `IP | MAC | Hostname`.
- `IP` should remain a stable fixed-width anchor column.
- `MAC` should remain visually aligned, but should not crowd `Hostname`
  unnecessarily.
- `Hostname` remains the trailing flexible column and should receive more usable
  leftover width than in the current styling.

### Typography

- Keep the existing desktop-oriented font baseline.
- Slightly strengthen header emphasis over row text.
- Preserve single-line clipped rendering for all three columns.

## Layout And Behavior Constraints

- The existing `ResultListData` shape remains unchanged.
- The reverse-DNS hostname update path remains unchanged.
- Rows must continue to accept empty hostnames.
- Column alignment between header and body must remain exact while scrolling.
- The result panel smoke tests and hostname-binding regression test must
  continue to pass after the visual refresh.

## Implementation Shape

Primary implementation target:

- `ui/main.slint`

Expected implementation areas:

- refine panel styling constants
- rebalance column width constants
- adjust header styling
- adjust row surface, separator, and spacing treatment
- preserve the stable test hooks already added for the hostname header and
  hostname cell text

Possible test touch points if required by the implementation:

- `tests/result_list_panel_smoke.rs`
- `tests/ui_smoke.rs`

The scan runtime, UI state reducer, and Rust-to-Slint data mapping should not
change for this task.

## Compatibility And Risk Considerations

- The hostname test hooks in `ui/main.slint` are now part of regression
  coverage and must remain stable unless the tests are updated in the same
  task.
- Column rebalance must not cause header/body drift.
- Any hover treatment should be skipped if it requires brittle Slint workarounds.
- The refreshed panel should still read well at the current minimum window
  width.

## Verification

Required verification for implementation:

1. `cargo check`
2. `cargo test --test result_list_panel_smoke --test ui_smoke -v`
3. any additional focused UI smoke tests added by the implementation

Manual acceptance checks:

1. The panel feels more deliberate and less flat.
2. The header is easier to distinguish from the data area.
3. `Hostname` no longer feels visually squeezed beside `MAC`.
4. Long result lists remain easy to scan vertically.
5. Blank hostname cells do not create noisy placeholder visuals.

## Future Extensions

Possible future work intentionally excluded from this design:

- sortable columns
- copyable rows
- row selection state
- vendor or interface metadata columns
- dedicated empty-state or no-results panel treatment
