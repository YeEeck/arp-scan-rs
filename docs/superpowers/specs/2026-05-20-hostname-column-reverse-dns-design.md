# Hostname Column Reverse DNS Design

## Summary

This design adds a `Hostname` column to the scan result list and fills it by
performing reverse DNS lookups for discovered hosts.

The selected interaction model is:

- scan discovery remains driven by ARP
- `IP` and `MAC` appear immediately when a host is found
- hostname lookup starts asynchronously after host discovery
- unresolved hostnames remain blank
- reverse DNS failures do not affect overall scan success or failure

The goal is to enrich the existing live result list without slowing the visible
scan flow or turning hostname resolution into a required part of scan
completion.

## Current Problems

- The result list only shows `IP` and `MAC`, which makes it harder to identify
  devices at a glance.
- The current scan pipeline has no place for host metadata that may arrive
  after the ARP hit itself.
- The UI state model currently assumes each discovered host row is complete at
  insertion time.

## Goals

- Add a third `Hostname` column to the result list.
- Keep the current live-scan behavior where hosts appear as soon as ARP
  discovery succeeds.
- Populate hostnames asynchronously after host discovery.
- Leave the hostname blank when reverse DNS produces no usable result.
- Ensure reverse DNS failures do not mark the scan as failed or partially
  failed.
- Prevent stale hostname results from an old scan from mutating rows in a newer
  scan.

## Non-Goals

- Changing the ARP discovery algorithm or its completion semantics.
- Blocking host row insertion on reverse DNS completion.
- Introducing vendor lookup, NetBIOS lookup, mDNS browsing, or other hostname
  discovery mechanisms.
- Adding visible per-row loading indicators or error states for hostname
  resolution.
- Reordering the list based on hostname.

## Chosen Interaction Model

### Discovery flow

When ARP discovery finds a host:

1. Insert the row immediately with:
   - `IP`
   - `MAC`
   - empty `Hostname`
2. Start a separate background reverse DNS lookup for that IP.
3. If the lookup resolves to a hostname, update only that row’s `Hostname`
   field.
4. If the lookup fails, times out, or returns no name, keep the field empty.

### Why this model

- It preserves the fast, streaming feel of the current UI.
- It keeps hostnames as optional enrichment rather than a gating dependency.
- It maps cleanly onto the current event-driven state reducer by treating
  hostname resolution as a later row update.

## Data And Event Model

### Result row shape

The result-row structures gain a `hostname` string field:

- Slint `ResultListData`
- Rust `ResultRow`

The default value for new discoveries is an empty string.

### New event type

The scan/runtime event model gains a new event for hostname enrichment.

Required payload:

- `task_id`
- `ip`
- `hostname`

The event represents a successful reverse DNS result only. Failure or absence
of a hostname should not emit a user-visible failure event.

### Stale event protection

The current `task_id` filtering rule must apply to hostname events too.

This ensures:

- hostname results from a cancelled or replaced scan are ignored
- rows from the active scan are the only rows eligible for hostname updates

## UI State Behavior

### Row insertion

On `HostFound`:

- insert or update the row with `IP`, `MAC`, and empty hostname

### Hostname update

On hostname-resolution event:

- find the row by IP key using the existing sorted row structure
- update only the `hostname` field
- preserve row order and existing `IP`/`MAC` values

### Failure semantics

Reverse DNS lookup failure must:

- not change status text
- not change progress text
- not emit scan failure
- not remove or replace the discovered row

## Result List Presentation

### Column order

The approved order is:

- `IP`
- `MAC`
- `Hostname`

### Layout

- The current table-like layout remains in place.
- `IP` and `MAC` keep explicit width control similar to the current design.
- `Hostname` becomes the flexible trailing column and consumes the remaining
  width.
- All three columns remain single-line and use clipping rather than wrapping.

### Empty hostname display

- If hostname lookup yields no usable value, the cell remains blank.
- No `N/A`, `-`, error text, or placeholder loading text is shown.

## Runtime Behavior

### Lookup timing

- Reverse DNS starts after ARP host discovery, not before and not after full
  scan completion.
- Lookup should run asynchronously so the scan pipeline can continue streaming
  results.

### Scan outcome independence

- ARP scan completion remains the only source of `Finished`, `Cancelled`, or
  `Failed` scan status.
- Reverse DNS is best-effort enrichment only.

### Concurrency considerations

- Hostname lookup work should not block UI updates for newly found hosts.
- The design may use a separate background task per discovered host, or a small
  helper path that asynchronously emits hostname events, as long as it does not
  alter scan correctness semantics.

## Implementation Shape

Primary files expected to change:

- `ui/main.slint`
  - add the `hostname` field to `ResultListData`
  - add the `Hostname` column to the result panel
- `src/ui_state.rs`
  - extend `ResultRow`
  - support hostname row updates
- `src/main.rs`
  - map hostname values into Slint rows
  - forward hostname update events to the UI
- `src/scan_master/runtime.rs`
  - define and emit hostname-resolution events
  - trigger reverse DNS lookups for discovered hosts
- `src/scan_master.rs`
  - keep exported event types aligned if needed

Expected tests to expand:

- `tests/ui_state.rs`
- `tests/scan_runtime.rs`
- `tests/result_list_panel_smoke.rs`
- `tests/ui_smoke.rs`

## Compatibility And Risk Considerations

- Reverse DNS behavior can vary across networks and environments, so tests
  should avoid relying on real network hostname resolution.
- Hostname resolution must be abstractable or injectable enough for deterministic
  tests.
- If a hostname result arrives after the scan finishes, it may still update the
  finished scan’s visible rows as long as the task is still the active task.
- If a new scan starts before a previous hostname result arrives, the stale
  result must be ignored.

## Verification

Required verification for implementation:

1. `cargo check`
2. `cargo test -v`

Manual acceptance checks:

1. Discovered hosts still appear immediately with `IP` and `MAC`.
2. The `Hostname` column exists and stays in the order `IP | MAC | Hostname`.
3. Some rows may fill in hostname slightly later than the ARP hit arrives.
4. Rows with no reverse DNS result remain blank in the hostname cell.
5. Hostname lookup failures do not change scan completion status.
6. Starting a new scan does not let old hostname results leak into the new list.

## Future Extensions

Possible follow-up work intentionally excluded from this design:

- visible hostname lookup progress indicators
- per-row retry controls
- vendor / NetBIOS / mDNS enrichment
- configurable hostname lookup timeout or toggle
