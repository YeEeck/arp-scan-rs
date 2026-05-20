# IP And Mask Input Toggle Design

## Summary

This design adds a user-facing input mode switch for scan target entry.

The selected interaction model is:

- the window defaults to `IP + subnet mask` mode
- users can switch between `CIDR` and `IP + subnet mask`
- switching attempts to convert the current value and preserve it
- scanning always runs from a normalized CIDR string internally
- `IP + subnet mask` accepts host addresses and normalizes them to the
  corresponding network CIDR before scan start

The goal is to make scan target entry more approachable without expanding the
scan runtime API beyond its current CIDR-based contract.

## Current Problems

- The current UI exposes only a single `CIDR` input field.
- CIDR notation is compact but not the most familiar input format for many
  users.
- The scan start path reads the window `cidr` property directly, so there is no
  dedicated place to normalize or convert alternative input formats.

## Goals

- Add a visible input mode switch between `CIDR` and `IP + subnet mask`.
- Default the UI to `IP + subnet mask`.
- Preserve user intent when switching modes by automatically converting valid
  values in either direction.
- Normalize `IP + subnet mask` input to a scan-ready CIDR string before
  calling the existing scan runtime.
- Allow host addresses in `IP + subnet mask` mode and normalize them to the
  containing network CIDR.
- Keep scan start failure handling integrated with the existing `Failed: ...`
  status path.

## Non-Goals

- Changing the runtime scan API to accept a new target type.
- Supporting IPv6 input.
- Adding advanced validation hints, inline per-field error labels, or partial
  live validation states beyond the existing start-failure model.
- Changing host iteration semantics or CIDR parsing rules in the scan runtime.

## Chosen Interaction Model

### Default mode

The window opens in `IP + subnet mask` mode.

Visible fields:

- `IP`
- `Subnet Mask`

The `CIDR` field is hidden until the user switches modes.

### Mode switching

The input area includes a mode selector that toggles between:

- `IP + subnet mask`
- `CIDR`

When the current mode contains a valid value:

- switching from `CIDR` converts to `IP` and dotted-decimal `Subnet Mask`
- switching from `IP + subnet mask` converts to a normalized CIDR string

When the current mode contains an invalid value:

- switching modes does not erase that mode’s text
- the destination mode keeps its last known text values
- no forced correction happens during switching

### Scan behavior

Regardless of the visible mode, pressing `Scan` first attempts to produce a
normalized CIDR string.

Examples:

- `192.168.1.0/24` stays `192.168.1.0/24`
- `192.168.1.23` + `255.255.255.0` becomes `192.168.1.0/24`
- `192.168.1.23` + `255.255.255.128` becomes `192.168.1.0/25`

If normalization fails, the existing start-failure UI path remains responsible
for surfacing the error.

## Data And Conversion Model

## Input state

The UI/Rust boundary gains explicit fields for the two input modes rather than
overloading one string.

Required state:

- input mode enum/value
- CIDR text
- IP text
- subnet mask text

The selected mode controls which controls are shown, but all three text values
remain available so the app can preserve the last known values across mode
switches.

### Conversion responsibilities

A dedicated input conversion helper is responsible for:

- validating CIDR text
- validating IPv4 text
- validating dotted-decimal subnet masks
- converting prefix length to dotted-decimal subnet mask
- converting dotted-decimal subnet mask to prefix length
- deriving the network address from host IP + subnet mask
- producing the canonical CIDR string used by the scan runtime

This keeps format logic out of `main.rs` and avoids scattering conversion rules
across UI callbacks.

### Subnet mask rules

Accepted subnet masks must be valid contiguous IPv4 masks.

Examples of valid masks:

- `255.255.255.0`
- `255.255.255.128`
- `255.255.255.255`
- `0.0.0.0`

Examples of invalid masks:

- `255.0.255.0`
- `255.255.255.1`
- non-IPv4 text

### Canonicalization

Normalization should emit a network-based CIDR string:

- compute prefix length from the subnet mask
- bitwise-AND the IP with the mask
- emit `<network>/<prefix>`

This means `IP + subnet mask` mode accepts a host address as input, but scanning
always starts from the network CIDR that contains that host.

## UI Structure

## Main input group

The single `CIDR` group box becomes a scan target input group with:

- a mode selector
- one conditional `CIDR` field
- or two conditional fields for `IP` and `Subnet Mask`

The rest of the page structure remains unchanged:

- `Scan` button
- `Cancel` button
- status text
- progress text
- result list panel

### Control behavior

- Input controls remain disabled while a scan is active, matching the current
  `scan_enabled` behavior.
- The mode selector follows the same disabled state during active scans.
- Placeholders should stay explicit and familiar:
  - CIDR example: `192.168.1.0/24`
  - IP example: `192.168.1.23`
  - Subnet mask example: `255.255.255.0`

## Integration With Existing Scan Flow

The scan runtime remains CIDR-driven.

The start flow becomes:

1. Read window mode and visible text fields.
2. Build an input conversion state/value from those texts.
3. Ask it for a normalized CIDR string.
4. On success, call `start_scan(&cidr, MAX_IN_FLIGHT)`.
5. On failure, route the error through `fail_to_start(...)` and keep the
   existing result-clearing/status behavior.

This keeps the change isolated to the UI/input boundary and does not require
runtime event or host enumeration changes.

## Error Handling

The current app already reports start failures by showing `Failed: ...`.

The new input path should continue using that model.

Expected invalid-start cases include:

- malformed CIDR text
- malformed IP text
- malformed subnet mask text
- non-contiguous subnet mask

The exact wording can stay simple and technical, as long as it clearly tells
the user the input is invalid.

## Implementation Shape

Primary files expected to change:

- `ui/main.slint`
  - replace the single `cidr` binding with explicit mode and text properties
  - add the mode selector
  - conditionally show the CIDR field or the IP/mask fields
- `src/main.rs`
  - read the new window properties
  - normalize current input to CIDR before calling `start_scan`
- `src/ui.rs`
  - regenerated bindings reflect the updated Slint properties
- `src/scan_master/ip_box.rs` or a new focused UI helper module
  - host the shared conversion primitives if reuse is clean
  - otherwise keep UI-focused normalization in a dedicated helper module

Expected tests to expand:

- `tests/ui_smoke.rs`
- `tests/ip_range.rs` if shared conversion helpers are added there
- a new focused conversion test file if input normalization lives outside the
  existing scan range helpers

## Testing Strategy

### Conversion tests

Add deterministic tests for:

- CIDR to IP + mask conversion
- IP + mask to CIDR conversion
- host-address normalization to network CIDR
- invalid subnet mask rejection
- invalid IP rejection
- invalid CIDR rejection in the CIDR path

### UI binding tests

Update generated-binding smoke coverage to assert:

- the new input properties exist
- the default mode is `IP + subnet mask`
- setting and reading CIDR/IP/mask fields works through bindings

### Start-flow coverage

Add or extend a focused test around scan start preparation so there is evidence
that `IP + subnet mask` input becomes the expected CIDR string before the scan
runtime is called.

## Compatibility And Risk Considerations

- Slint conditional layout changes can affect control spacing, so the new input
  group should preserve the current overall page rhythm.
- If conversion helpers are placed inside the scan range module, keep the
  responsibility boundary clear so UI convenience logic does not leak into scan
  iteration behavior.
- Preserving invalid text across mode switches must not accidentally overwrite a
  previously valid value in the other mode.

## Verification

Required verification for implementation:

1. `cargo check`
2. `cargo test -v`

Manual acceptance checks:

1. The window opens in `IP + subnet mask` mode.
2. Switching from valid CIDR to `IP + subnet mask` backfills both fields.
3. Switching from valid `IP + subnet mask` to CIDR backfills the CIDR field.
4. Entering a host IP with a valid subnet mask scans the normalized network
   CIDR.
5. Invalid IP, mask, or CIDR input fails through the existing start failure
   path.
6. Input controls and mode switching remain disabled while a scan is active.

## Future Extensions

Possible follow-up work intentionally excluded from this design:

- inline field-level validation messaging
- automatic live normalization while typing
- remembering the last selected input mode across app restarts
- IPv6 scan target input
