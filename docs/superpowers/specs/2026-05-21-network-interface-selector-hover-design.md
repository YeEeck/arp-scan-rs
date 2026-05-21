# Network Interface Selector Hover Design

## Summary

This design adds a lightweight hover effect to the network interface selector's
popup rows in the Slint desktop UI.

The chosen direction is intentionally subtle:

- only selectable rows react to pointer hover
- the reaction is a soft background-color shift
- the background change animates smoothly to match the current restrained UI

Selection logic, disabled-item behavior, popup structure, and Rust-side data
flow remain unchanged.

## Current Problem

- The custom popup list for network interfaces has selected and disabled visual
  states, but no hover feedback.
- When users move the pointer across selectable rows, nothing changes visually,
  so the list feels less responsive than the rest of the UI.

## Goals

- Add visible but understated hover feedback to selectable popup rows.
- Keep the interaction aligned with the current cool gray-blue palette.
- Preserve the stronger selected state so hover does not compete with it.
- Leave disabled rows visually inert.

## Non-Goals

- Changing how items are selected.
- Adding keyboard-navigation behavior.
- Restyling the trigger button or unrelated controls.
- Adding stronger emphasis such as borders, shadows, or motion on text.

## Chosen Visual Direction

The approved direction is the lightweight option:

- selectable rows fade from white to a very light gray-blue hover background
- the transition uses a short background animation
- selected rows keep their existing selected background
- disabled rows keep their current disabled surface and opacity

This keeps the component consistent with the existing utility-style visual
language while still making pointer movement feel acknowledged.

## Implementation Shape

The primary implementation target is:

- `ui/main.slint`

Expected changes:

- expose hover state from each row's `TouchArea`
- compute row background from selected, hover, enabled, and default states
- animate background transitions so hover-in and hover-out feel smooth

No Rust files or UI-facing data structures need to change.

## Testing And Verification

Required verification:

1. `cargo check`
2. `cargo test -v`

Testing note:

- The current Slint test setup in this repository provides solid click/smoke
  coverage, but this change is primarily visual and does not have a low-risk
  automated hover assertion path in the existing tests.
- Verification therefore relies on successful build/test regression coverage
  plus manual visual confirmation when the UI is run.

## Manual Acceptance Checks

1. Moving the mouse over a selectable interface row changes only that row's
   background.
2. The hover background is softer than the selected background.
3. Moving the mouse away returns the row to its normal background smoothly.
4. Disabled rows do not show hover feedback.
5. Clicking behavior and popup closing behavior remain unchanged.
