# Window Resize And Windows Typography Design

## Summary

This design fixes three UI issues in the current Slint desktop window:

1. The result list can visually overflow the window bounds.
2. Text rendering and density look weak on a Windows 1080p desktop.
3. The window should support free resizing, with the result area consuming most
   of the added or removed space.

The scope is intentionally limited to layout and presentation behavior. The scan
engine, event flow, and runtime logic remain unchanged.

## Current Problems

- The result container uses a fixed height that can fight the overall window
  layout and allow content to push beyond the visible bounds.
- The current typography and spacing are too tight for a Windows 1080p desktop
  UI, which makes the interface look cramped and less legible.
- The window does not intentionally define resize behavior, minimum usable
  bounds, or how each section should respond when the window grows or shrinks.

## Goals

- Allow the main window to be freely resized by the user.
- Ensure the result area fills the remaining vertical space and never draws
  outside its container.
- Keep the top controls usable while the result list absorbs most size changes.
- Improve text clarity on Windows by defining a more deliberate font family and
  size hierarchy.
- Keep result rows single-line for readability.
- Preserve existing scan behavior and data flow.

## Non-Goals

- Rewriting the result area into a custom table widget.
- Adding filtering, search, export, or sorting controls.
- Changing scan orchestration, concurrency, or cancellation logic.
- Introducing per-platform feature divergence beyond font selection and visual
  tuning.

## Chosen Interaction Model

### Window resizing

- The main window becomes user-resizable.
- The UI defines a minimum width and minimum height to prevent unusable
  collapse.
- Extra vertical space primarily goes to the result list region.
- Reduced vertical space is taken primarily from the result list region, while
  the top form area remains structurally stable.

### Top area behavior

The top area remains composed of:

- CIDR input section
- button row
- status text
- progress text

These elements keep approximately fixed heights and only expand horizontally as
the window width changes.

### Result area behavior

- The result area becomes the single flexible section in the layout.
- It fills the remaining window height below the top controls.
- It owns scrolling and clipping, so list contents never paint outside the
  visible frame.
- The current fixed-height approach is removed.

## Typography And Density

The UI will use a clearer desktop-oriented visual scale.

### Typography goals

- Increase baseline readability on Windows 1080p.
- Establish visible hierarchy between section titles, control text, status
  text, table header text, and row text.
- Avoid over-dense row spacing that makes the list feel cramped.

### Windows font strategy

- On Windows, define an explicit UI font family instead of relying on an
  unstable default fallback chain.
- The chosen family should match common Windows desktop UI expectations and be
  suitable for Latin text rendering used by CIDR, IP, and MAC data.
- If the preferred family is unavailable, the UI may fall back to Slint or
  system defaults, but the design should optimize for standard Windows
  installations.

### Size and spacing strategy

- Increase outer padding around the main content.
- Increase vertical spacing between major sections.
- Increase input height and button height to a desktop-appropriate size.
- Increase table header height and row height.
- Keep status and progress text readable without visually dominating the layout.

These values should be centralized as UI constants so later tuning does not
require hunting through the layout tree.

## Result List Presentation

- The `IP` and `MAC` columns remain single-line.
- Neither column wraps.
- Readability is prioritized over squeezing all content into narrow widths.
- In narrow windows, content may be clipped rather than wrapped.
- The list should remain vertically scrollable inside its own bounded region.

## Implementation Shape

The primary implementation target is:

- `ui/main.slint`

Expected changes:

- define explicit window minimum size
- enable intentional resize behavior
- replace the fixed-height result container with a fill-remaining-space layout
- introduce centralized spacing, sizing, and typography constants
- improve table header and row sizing
- keep column content single-line and layout-stable

Rust-side changes are out of scope unless a minimal compatibility adjustment is
required by Slint API usage.

## Error And Compatibility Considerations

- The resize behavior must not break existing scan interactions.
- Result rendering must continue to work with the existing `VecModel` updates.
- Narrow windows should degrade by clipping content, not by overlapping or
  escaping the container.
- The design should avoid assumptions that require Windows-only build-time
  resources beyond standard system fonts.

## Verification

Required verification for implementation:

1. `cargo check`
2. `cargo test -v`
3. successful Slint/UI build as part of normal Rust compilation

Manual acceptance checks:

1. The window can be freely resized.
2. The result list no longer overflows the window bounds.
3. Enlarging the window mostly expands the result area.
4. Shrinking the window keeps the top controls usable and the list confined to
   its scroll region.
5. Text on Windows 1080p is visibly clearer and less cramped than before.

## Future Extensions

Possible follow-up work that is intentionally not included in this design:

- richer table visuals such as zebra striping or stronger headers
- configurable font scaling
- adjustable column widths or a true resizable table
- horizontal scrolling for very narrow windows
