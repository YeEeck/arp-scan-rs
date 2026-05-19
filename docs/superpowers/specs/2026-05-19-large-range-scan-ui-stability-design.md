# Large-Range Scan UI Stability Design

## Summary

This design upgrades the current Slint UI and scan orchestration so the application remains usable for large ranges such as `/16`, while still delivering real-time results. The focus is on four behaviors:

1. Allow large-range scans instead of rejecting them outright.
2. Cap concurrent in-flight ARP probes so thread count does not grow without bound.
3. Show progress and status in the UI while results stream in.
4. Allow cancellation while preserving already found hosts until the next scan starts.

The design does not add export, filtering, search, or multi-task scanning.

## Current Problems

The current implementation has several structural issues:

- The UI thread runs the full scan synchronously, freezing the window until the scan completes.
- The scan creates one thread per target host with no concurrency limit.
- Results are appended in thread completion order, so the list is unstable and unsorted.
- Errors are printed to stdout even though the GUI binary suppresses the console window.
- The result model stores one display string instead of structured host data, which prevents stable sorting and richer UI states.

## Goals

- Keep the UI responsive during scanning.
- Support large ranges such as `/16`.
- Stream discovered hosts into the UI in real time.
- Keep results deduplicated and sorted by IP while streaming.
- Show clear scan lifecycle states: idle, scanning, cancelled, finished, failed.
- Allow the user to cancel a scan and keep already found hosts visible.
- Clear previous results when a new scan starts.

## Non-Goals

- CLI and GUI mode unification.
- Exporting scan results.
- User-configurable concurrency controls in the first iteration.
- Running multiple scans at the same time.
- Advanced filtering, search, or result grouping.

## User-Facing Behavior

### Scan start

When the user clicks `Scan`:

- The previous result list is cleared immediately.
- The CIDR input is locked.
- The `Scan` button is disabled.
- The `Cancel` button becomes enabled.
- Status changes to `Scanning`.
- Progress starts at `0 / total_hosts`.

### During scan

- Results appear as soon as a host is found.
- Results remain deduplicated and sorted by IP.
- Progress updates continuously as probes complete.
- The UI remains responsive.

### Cancel

When the user clicks `Cancel`:

- The current scan stops dispatching new probes.
- Already running probes are allowed to finish naturally.
- Any discovered hosts already emitted remain visible.
- The final state changes to `Cancelled`.
- The CIDR input becomes editable again.
- The `Scan` button becomes enabled again.

### Finish

When the scan completes normally:

- The final state changes to `Finished`.
- The UI shows final counts for scanned hosts and discovered hosts.
- The CIDR input becomes editable again.

### Failure

If the scan setup or runtime fails:

- The final state changes to `Failed`.
- The UI shows a human-readable error message.
- Any already collected results remain visible.
- The CIDR input becomes editable again.

## Architecture

The implementation will be split into three layers.

### 1. UI state layer

This layer owns Slint-facing state:

- `cidr`
- `is_scanning`
- `can_cancel`
- `status_text`
- `progress_text`
- `result_rows`

The UI does not perform scanning work directly. It only reacts to state changes.

### 2. Scan controller layer

This layer sits between the UI and the scan engine. It is responsible for:

- starting a new scan task
- cancelling the active scan
- receiving scan events from background threads
- ignoring late events from stale tasks
- translating scan events into UI model updates

This layer owns the active `task_id` and the cancellation handle for the current scan.

### 3. Scan engine layer

This layer performs the scan and emits events. It is responsible for:

- expanding the CIDR range
- counting total probe targets
- enforcing an upper bound on concurrent in-flight probe threads
- probing each host
- reporting progress and discovered hosts
- shutting down cleanly on cancellation

The scan engine must not depend on Slint types.

## Scan Execution Model

The design intentionally avoids a traditional small fixed-size thread pool because each ARP probe blocks while waiting, which would turn the pool size into a direct scan throughput limit.

Instead, the scan uses a bounded in-flight window:

- The controller starts one background dispatcher thread for the scan.
- The dispatcher iterates the target IP range.
- For each target IP, it checks whether the number of active probe threads is below `max_in_flight`.
- If capacity exists, it spawns a short-lived probe thread for that IP.
- If capacity is full, it waits until one or more probe threads complete, then continues.
- Each probe thread runs one ARP check and exits.

This model keeps the high parallelism needed for blocking ARP probes while preventing the unbounded thread explosion of the current implementation.

### Concurrency bound

The first iteration will use a fixed Rust-side constant such as `256` or `512` for `max_in_flight`.

The value will not be user-configurable in the first iteration. The design should centralize the constant so it can be tuned later without changing UI behavior.

## Task Identity And Event Safety

Each scan gets a unique `task_id`.

All emitted events carry that `task_id`. The controller only applies events for the current active task. This prevents:

- results from a cancelled scan leaking into a newly started scan
- late-arriving completions from updating a reset UI

This is required because cancellation is cooperative and some in-flight probes may complete after the user has already started a new scan.

## Scan Events

The scan engine emits a typed event stream. The event set for the first iteration is:

- `Started { task_id, total_hosts }`
- `Progress { task_id, scanned_hosts, total_hosts }`
- `HostFound { task_id, ip, mac }`
- `Finished { task_id, scanned_hosts, found_hosts }`
- `Cancelled { task_id, scanned_hosts, found_hosts }`
- `Failed { task_id, message }`

Behavioral rules:

- `Started` is emitted once.
- `Progress` is emitted after each completed probe or in small batches if UI churn becomes excessive.
- `HostFound` is emitted only for successful host detections.
- `Finished`, `Cancelled`, and `Failed` are terminal events.

## Result Storage, Sorting, And Deduplication

The UI must not use a plain string list as the source of truth.

Rust-side result state will use an ordered keyed structure, such as `BTreeMap<u32, HostRow>`, where the key is the numeric IPv4 address.

This gives the required behavior:

- deduplication: inserting the same IP twice overwrites or ignores duplicates
- stable ordering: rows always render in ascending IP order
- real-time updates: each new host can be merged into the ordered map immediately

The displayed row model should expose at least:

- `ip`
- `mac`

Optional future fields such as `vendor` or `response_state` can be added later without redesigning the model shape.

## Cancellation Model

Cancellation is cooperative.

The active task owns an atomic cancellation flag:

- clicking `Cancel` sets the flag
- the dispatcher stops spawning new probe threads once the flag is observed
- already-running probe threads are allowed to finish naturally
- when all in-flight work drains, the scan emits `Cancelled`

The design does not attempt forceful thread termination. Rust threads cannot be safely killed, and forced cancellation would complicate correctness without providing a strong benefit here.

## Error Handling

Errors must be surfaced to the UI instead of stdout.

Error sources to account for:

- invalid CIDR input
- scan setup failure
- channel or coordination failure inside the controller
- probe-level ARP failures that should not kill the full scan unless they indicate a systemic problem

Design rules:

- invalid CIDR should fail fast before starting the scan
- individual host probe failures should usually count as scanned-but-not-found, not as fatal task failure
- fatal task failures should emit `Failed { message }`
- UI status text should contain a human-readable message

## UI Design

The first iteration updates structure and logic more than visual styling.

### Required UI sections

- CIDR input area
- `Scan` button
- `Cancel` button
- status text area
- progress text area
- result list or table with separate `IP` and `MAC` columns

### UI states

#### Idle

- CIDR editable
- `Scan` enabled
- `Cancel` disabled
- existing results remain visible from the last run

#### Scanning

- CIDR disabled
- `Scan` disabled
- `Cancel` enabled
- status shows scanning
- progress updates continuously

#### Cancelled

- CIDR editable
- `Scan` enabled
- `Cancel` disabled
- results remain visible
- status shows cancelled

#### Finished

- CIDR editable
- `Scan` enabled
- `Cancel` disabled
- results remain visible
- status shows final counts

#### Failed

- CIDR editable
- `Scan` enabled
- `Cancel` disabled
- results remain visible
- status shows error message

## Module Changes

The current code will need a modest refactor.

### `src/main.rs`

- remove direct synchronous scan execution from the Slint callback
- initialize richer UI state
- delegate scan actions to the controller

### `ui/main.slint`

- replace the single-string result row with a structured row model
- add UI state bindings for status, progress, and cancel availability
- present separate IP and MAC fields

### `src/scan_master.rs`

- stop returning only a final `Vec<IpCheckResult>` for UI-driven usage
- expose scan orchestration that can stream events
- enforce bounded in-flight concurrency

### New supporting modules

The design likely benefits from splitting responsibilities into focused modules, for example:

- controller logic
- scan event definitions
- result-row transformations
- probe trait or adapter for testability

Exact file names can be finalized in the implementation plan.

## Test Strategy

The main risk in this iteration is coordination logic rather than pure ARP probing. Tests should focus on orchestration behavior.

### Core scan tests

- CIDR expansion and total host counting are correct
- bounded in-flight concurrency is respected
- cancellation stops new dispatches
- terminal events are emitted correctly
- progress counts are monotonic and end in the correct totals

### Result state tests

- duplicate discoveries do not produce duplicate rows
- rows remain sorted by IP
- late events from stale tasks are ignored

### Controller/UI logic tests

- starting a scan clears the previous results
- scanning state disables input and enables cancel
- cancellation preserves already found rows
- a new scan after cancellation clears previous rows before receiving new ones
- fatal errors appear in UI state rather than stdout

## Probe Abstraction For Tests

The ARP probe call should be wrapped behind a narrow interface so tests can inject a fake implementation.

This is required to test:

- slow probes
- successful probes
- repeated duplicate hits
- task cancellation during active work
- bounded concurrency under load

Without this abstraction, the new orchestration would be difficult to test reliably.

## Rollout Scope

This design is intentionally scoped as one iteration:

- responsive UI
- bounded high-concurrency scan execution
- progress and status display
- cancel support
- real-time sorted deduplicated results
- basic structured error reporting

Anything beyond this should be deferred to a later spec.
