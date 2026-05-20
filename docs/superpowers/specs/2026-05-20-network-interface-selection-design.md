# Network Interface Selection Design

## Summary

This design adds network interface enumeration to the UI and lets users choose
an interface before scanning.

The selected interaction model is:

- the window shows a network interface selector inside the existing scan target
  group
- the selector lists all discovered interfaces, including unusable ones
- unusable interfaces stay visible but are disabled or labeled with a reason
- the selector defaults to no selection
- selecting an interface fills the existing scan target inputs using the
  current input mode
- scanning still starts from a normalized CIDR string and does not gain a new
  runtime interface parameter

The goal is to make scan target setup faster and safer without changing the
current scan runtime contract.

## Current Problems

- The UI requires manual scan target entry even when the local machine already
  knows the available interfaces and their IPv4 configuration.
- Users have to infer or retype the local interface address range before
  scanning.
- The current UI/input model has no concept of network interfaces or interface
  availability.

## Goals

- Enumerate current network interfaces and expose them in the UI.
- Show all discovered interfaces in the selector rather than hiding unusable
  ones.
- Mark interfaces that cannot provide a scan target as unavailable with a clear
  reason.
- Default the selector to no active selection.
- When a usable interface is selected, fill the visible scan target inputs
  according to the current input mode.
- Preserve the existing manual editing flow after interface-based autofill.
- Keep scan start behavior based on the already-normalized CIDR string.

## Non-Goals

- Changing the ARP scan runtime to bind packets to a selected interface.
- Automatically selecting an interface on startup.
- Refreshing interface state continuously while the window remains open.
- Supporting IPv6 interface-derived scan targets.
- Adding advanced interface metadata such as MTU, gateway, or DNS servers to
  the UI.

## Chosen Interaction Model

### Interface selector placement

The existing `Scan Target` group gains a new control row above the input mode
selector.

The new row contains a network interface `ComboBox`.

The selector remains part of the scan target setup rather than becoming a
separate settings panel because its only purpose in this design is to populate
the target fields the user already edits there.

### Default state

On startup:

- no interface is selected
- existing target input defaults remain unchanged
- no autofill runs until the user explicitly chooses a usable interface

The empty selector entry should read like an invitation, for example
`Select network interface`.

### Selection behavior

When the user selects a usable interface:

- if the current mode is `IP + subnet mask`, fill `IP` and `Subnet Mask`
- if the current mode is `CIDR`, fill the normalized CIDR for that interface’s
  IPv4 network

The current mode stays unchanged. Selecting an interface never forces a mode
switch.

After autofill, users can still edit any field manually. The selection is a
starting point, not a lock.

### Unusable interfaces

All discovered interfaces remain visible in the selector.

Interfaces are unusable for autofill when they do not provide enough IPv4
configuration to derive a scan target. Typical examples include:

- no IPv4 address
- no IPv4 subnet mask / prefix information
- loopback-only entries
- interface state that the implementation intentionally excludes from scanning

Unusable entries must be represented in a way users can understand at a glance:

- disabled when the UI toolkit supports it cleanly for the chosen control
- otherwise labeled with a short reason in the visible text and ignored by the
  selection handler

Examples:

- `Bluetooth Network Connection (No IPv4 address)`
- `Loopback Pseudo-Interface (Loopback)`

## Data Model

### Interface view model

The UI/Rust boundary gains a dedicated interface item model.

Each item needs:

- stable identifier or selection key
- display label
- availability flag
- unavailability reason when applicable
- optional IPv4 address
- optional IPv4 subnet mask
- optional derived CIDR text

The display label should be fully prepared on the Rust side so the Slint view
can bind it directly.

### Input autofill model

The existing scan target input state remains the source of truth for:

- input mode
- CIDR text
- IP text
- subnet mask text

Selecting an interface produces a new input state by filling whichever fields
belong to the active mode.

This means the new interface selection path becomes another caller of the
existing normalization and mode-conversion helpers, rather than introducing a
parallel target representation.

## Platform Behavior

### Windows

Windows is the primary supported scanning platform, so interface enumeration
must provide real system data there.

The implementation should read the current interface table and IPv4 unicast
configuration from Windows networking APIs, then project that into the UI model
described above.

The design deliberately keeps the Windows-specific API work behind a focused
module so the rest of the app consumes a platform-neutral interface list.

### Non-Windows behavior

The project already limits actual ARP scanning to Windows.

For non-Windows builds, interface enumeration should preserve buildability and
testability without pretending the feature is fully supported. Acceptable
behavior is:

- return an empty list and keep the selector effectively inert, or
- return a single unavailable placeholder item indicating the platform
  limitation

The important requirement is that non-Windows builds continue to compile and
tests can exercise the interface-selection state machinery deterministically.

## Availability Rules

An interface is considered usable when all of the following are true:

- it exposes an IPv4 address
- it exposes prefix or subnet mask information that can be converted into a
  valid contiguous IPv4 subnet mask
- it is not a loopback-only interface

If any rule fails, the item remains in the list but gains an explicit reason.

Reason text should stay concise and stable enough for tests, such as:

- `No IPv4 address`
- `Missing subnet mask`
- `Invalid subnet mask`
- `Loopback`
- `Unsupported platform`
- `Failed to load interfaces`

## UI Structure

### Scan target group

The `Scan Target` group now contains, in order:

1. network interface selector
2. input mode selector
3. conditional input fields for either `CIDR` or `IP + subnet mask`

The rest of the page remains unchanged:

- `Scan` button
- `Cancel` button
- status text
- progress text
- result list panel

### Disabled-state behavior

While a scan is active:

- the interface selector is disabled
- the input mode selector is disabled
- scan target text inputs remain disabled as they are today

This matches the existing `scan_enabled` behavior and avoids mid-scan target
mutation.

## Error Handling

Failure to enumerate interfaces should not crash the app or prevent manual
scanning.

If interface loading fails:

- keep the manual target inputs usable
- surface the issue through the interface selector content, not the scan status
  text
- avoid rewriting the current target fields

If a selection callback somehow receives an unusable item:

- ignore the autofill request
- preserve the current input values
- avoid changing scan status text

## Integration With Existing Input Flow

The scan start pipeline remains unchanged at a high level:

1. read the current input mode and target text fields
2. normalize them to CIDR
3. call `start_scan(&cidr, MAX_IN_FLIGHT)`

Interface selection happens strictly before this pipeline and only updates the
input fields that step 1 already reads.

This keeps the feature isolated to interface discovery, UI binding, and input
state transitions.

## Implementation Shape

Primary files expected to change:

- `ui/main.slint`
  - add the network interface selector and related bindings
- `src/main.rs`
  - load interface items at startup
  - bind the interface model into the window
  - handle selection-driven autofill using the current input mode
- `src/scan_target.rs`
  - add helpers that map a chosen interface’s IPv4 data into the existing input
    state without changing the active mode
- `src/ui.rs`
  - regenerated bindings reflect the new Slint properties
- `src/lib.rs`
  - export the new interface module if tests need it directly
- new focused module such as `src/network_interface.rs`
  - define platform-neutral interface item types
  - implement Windows enumeration
  - provide non-Windows fallback behavior

Expected tests to expand:

- `tests/ui_smoke.rs`
- `tests/scan_target.rs`
- a new focused interface-model test file, for example
  `tests/network_interface.rs`

## Testing Strategy

### Interface model tests

Add deterministic tests for:

- usable interface projection into autofill-ready data
- unavailable reason mapping for interfaces without IPv4
- unavailable reason mapping for loopback interfaces
- preserving all interfaces in the visible list even when some are unusable

### Input autofill tests

Add deterministic tests for:

- selecting a usable interface in `IP + subnet mask` mode fills only `IP` and
  `Subnet Mask`
- selecting a usable interface in `CIDR` mode fills only `CIDR`
- selecting an unusable interface leaves the prior input values unchanged
- switching modes after autofill still uses the existing conversion rules

### UI binding tests

Update generated-binding smoke coverage to assert:

- the new interface model/property exists
- the selected-interface property defaults to the empty choice
- setting interface display items through bindings succeeds

## Verification

Required verification for implementation:

1. `cargo check`
2. `cargo test -v`

Manual acceptance checks:

1. The window opens with no selected interface.
2. The selector lists both usable and unusable interfaces.
3. Unusable interfaces are clearly labeled and cannot trigger autofill.
4. Choosing a usable interface in `IP + subnet mask` mode fills those two
   fields.
5. Choosing a usable interface in `CIDR` mode fills the CIDR field and does not
   change the selected mode.
6. Manual edits after autofill still scan through the existing normalized CIDR
   path.
