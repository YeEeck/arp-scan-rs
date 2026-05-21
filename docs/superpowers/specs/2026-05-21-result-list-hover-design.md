# Result List Hover Design

## Summary

This design adds a lightweight hover effect to the scan result list rows in the
Slint desktop UI.

The approved direction matches the network interface selector hover treatment:

- full-row hover feedback
- only a soft background-color change
- a short, smooth transition

The existing result layout, zebra striping, separators, and data flow remain
unchanged.

## Current Problem

- The result list rows currently show only alternating zebra backgrounds.
- When the pointer moves across rows, there is no hover feedback, so the list
  feels less responsive than the interface selector and other interactive
  controls.

## Goals

- Add subtle full-row hover feedback to each visible result row.
- Keep the visual weight below the selected header band and below the stronger
  selector selected state.
- Preserve the current zebra striping as the default non-hover appearance.
- Keep the implementation local to the Slint view layer.

## Non-Goals

- Adding row selection, click actions, or keyboard navigation.
- Changing sorting, ordering, scrolling, or clipping behavior.
- Restyling the table header or outer panel container.
- Adding borders, shadows, or text animation to rows.

## Chosen Visual Direction

The chosen approach is a lightweight, full-row hover background:

- each row keeps its current zebra background when idle
- when hovered, the row background shifts to a very light gray-blue surface
- the hover color is close to the network interface selector hover color and
  may be slightly weaker to avoid competing with the panel header
- the change animates with a short background transition

This keeps the result list visually consistent with the selector while still
respecting the more data-dense table layout.

## Implementation Shape

The primary implementation target is:

- `ui/main.slint`

Expected changes:

- add a `TouchArea` per rendered result row
- derive each row background from hover state plus the current zebra fallback
- animate row background transitions

The row text layout, divider line, and column widths remain unchanged.

## Testing And Verification

Required verification:

1. `cargo check`
2. `cargo test -v`

Testing note:

- This change is presentational and the current repository test setup does not
  provide a low-risk automated assertion path for hover visuals.
- Verification therefore relies on existing build/test regression coverage plus
  manual visual confirmation when the UI is run.

## Manual Acceptance Checks

1. Moving the mouse over a result row changes that row background only.
2. Hover feedback covers the full visible row width.
3. The hover treatment is subtler than the header band and does not overpower
   the zebra striping.
4. Moving the mouse away returns the row to its original zebra background
   smoothly.
5. Scrolling, clipping, and column alignment remain unchanged.
