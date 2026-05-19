# Result List Panel Styling Design

## Summary

This design refreshes only the scan result list presentation in the Slint
desktop UI.

The selected direction is a balanced-density tool panel style:

- more structured than the current plain bordered box
- still restrained and utility-focused
- easier to scan across long result sets

The scan workflow, result data model, window layout behavior, and Rust-side UI
state logic remain unchanged.

## Current Problems

- The result area looks like a generic white rectangle instead of an intentional
  part of a network utility UI.
- The header is visually weak, so the column labels do not clearly separate
  themselves from the result rows.
- The list rows have limited visual structure, which makes long scans harder to
  read quickly.
- The overall container, spacing, and typography feel serviceable but not
  polished.

## Goals

- Make the result area feel like a deliberate tool panel rather than a default
  placeholder box.
- Improve column and row readability without lowering information density too
  far.
- Keep a balanced density: more breathing room than the current version, while
  still showing a practical number of hosts per viewport.
- Preserve the current two-column table shape and scrolling behavior.
- Keep the overall look restrained so it still fits a desktop utility.

## Non-Goals

- Replacing the current custom header plus `ListView` structure.
- Adding sorting, filtering, selection, copy actions, or context menus.
- Changing scan progress behavior, result ordering, or data flow.
- Restyling the CIDR input, buttons, status text, or wider window chrome beyond
  what is needed to keep the list visually consistent.

## Chosen Visual Direction

### Selected style

The approved direction is a balanced-density panel style derived from the
"tool panel" option explored during brainstorming:

- not card-based
- not highly decorative
- not aggressively dense

It should feel closer to a professional desktop utility than to a dashboard or
marketing UI.

### Why this direction

- It matches the existing app purpose: scan, inspect, and read structured host
  output.
- It improves polish without visually overpowering the input and control area.
- It can be implemented with low technical risk inside the current Slint layout
  structure.

## Visual Design Rules

### Container

- Keep a single bounded result container.
- Use a cleaner border treatment than the current flat rectangle.
- Add moderate corner rounding so the panel feels intentional, but keep it
  tighter than a card UI.
- Preserve a white content surface for clarity.

### Header

- Turn the header into a distinct shallow band with a subtle cool-gray or
  gray-blue tint.
- Increase header text emphasis slightly through weight and contrast.
- Keep the header visually aligned with the scrollable columns.
- Avoid heavy shadows or saturated accent colors.

### Rows

- Keep each result row as a simple table row, not a separate card.
- Use balanced row height and left-right padding to improve legibility.
- Add light row separation through very soft dividers, zebra striping, or both.
- If supported cleanly in the current Slint version, add a subtle hover state.
- Any hover treatment must remain understated and must not compete with the
  header.

### Density

- Use a balanced density target:
  - more breathing room than the current list
  - less vertical expansion than a card-style layout
- Prioritize fast scanning of IP and MAC values over dramatic spacing.

### Typography

- Keep the existing desktop-oriented font baseline.
- Slightly strengthen hierarchy between header text and row text.
- Keep row content single-line and clipped rather than wrapped.

## Implementation Shape

The primary implementation target is:

- `ui/main.slint`

Expected changes:

- introduce or refine result-list styling constants for border, corner radius,
  header background, row spacing, and row visuals
- restyle the outer result panel container
- restyle the custom header band
- adjust row height, row padding, and column spacing to the approved balanced
  density
- add lightweight row differentiation while preserving the current scrolling
  structure

The `ResultListData` structure, callbacks, Rust-side state bindings, and scan
runtime remain unchanged.

## Compatibility And Risk Considerations

- The header must remain locked to the same horizontal scroll position as the
  list body.
- Visual changes must not break clipping or scrolling behavior.
- If row hover is awkward or unreliable in the current Slint version, prefer
  stable zebra striping and separators over forcing hover behavior.
- The refreshed styling should still look acceptable on Windows systems using
  the current font stack.

## Verification

Required verification for implementation:

1. `cargo check`
2. `cargo test -v`
3. successful Slint/UI build as part of normal Rust compilation

Manual acceptance checks:

1. The result area reads as a distinct tool panel rather than a plain box.
2. The header is easier to distinguish from the data rows.
3. Row spacing feels more polished while still fitting a practical number of
   hosts on screen.
4. Long result sets remain easy to scan vertically.
5. Scrolling, clipping, and column alignment still behave correctly.

## Future Extensions

Possible follow-up work that is intentionally excluded from this design:

- sortable columns
- row selection and copy affordances
- vendor lookup or additional metadata columns
- sticky status summary inside the result panel
