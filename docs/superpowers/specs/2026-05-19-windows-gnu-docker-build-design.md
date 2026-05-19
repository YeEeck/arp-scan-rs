# Windows GNU Docker Build Design

## Summary

This design adds a repeatable Docker-based cross-compilation path for producing a
`windows-x64` executable from the Linux development environment without
installing Windows toolchains on the host.

The first iteration focuses on one concrete outcome:

1. Build `arp-scan-rs.exe` for `x86_64-pc-windows-gnu` inside Docker.
2. Keep the host environment clean by isolating toolchain setup in the image.
3. Export the built executable into a stable path inside the repository.
4. Make the workflow easy to rerun locally and later reuse in CI.

The design does not replace the existing native Linux workflow and does not add
Windows runtime smoke testing in this iteration.

## Current Problems

The current repository can only build for the host target:

- The host only has `x86_64-unknown-linux-gnu` installed.
- No Windows GNU Rust target is installed on the host.
- No MinGW cross-linker is installed on the host.
- The project has no repository-owned cross-build workflow, so builds depend on
  ad-hoc local machine setup.

As a result, contributors cannot reliably produce a Windows executable from the
current Linux environment.

## Goals

- Produce `arp-scan-rs.exe` for `x86_64-pc-windows-gnu`.
- Keep all cross-build dependencies inside Docker.
- Make the workflow repeatable with one repository-owned script.
- Export the executable to a stable repository path.
- Preserve build logs and failures from `cargo build` so diagnosis is direct.

## Non-Goals

- Supporting `x86_64-pc-windows-msvc`.
- Supporting `i686-pc-windows-gnu`.
- Automatically running the Windows executable under Wine.
- Packaging installers, zip bundles, or release metadata.
- Replacing CI or release automation.

## User-Facing Workflow

The user runs one command from the repository root:

```bash
./scripts/build-windows-gnu.sh
```

Expected behavior:

1. Docker builds or reuses a dedicated builder image.
2. The image compiles the project for `x86_64-pc-windows-gnu` in `--release`
   mode.
3. The script copies `arp-scan-rs.exe` out of the container.
4. The executable is written to `dist/windows-x64/arp-scan-rs.exe`.
5. Temporary containers are removed automatically.

On success, the script prints the final artifact path.

On failure, the script exits non-zero and leaves the image in place for faster
retries.

## Repository Layout

The first iteration adds these repository-owned files:

- `docker/windows-gnu/Dockerfile`
- `scripts/build-windows-gnu.sh`
- `.dockerignore`

The first iteration also standardizes this generated output path:

- `dist/windows-x64/arp-scan-rs.exe`

The `dist/` directory is an output location, not source of truth, so generated
executables should not be committed.

## Docker Image Design

The Docker image will be purpose-built for this repository and target.

Base image:

- Use an official Rust image, such as `rust:1.95-bookworm`, to avoid relying on
  host Rust installation details.

Image responsibilities:

- install `mingw-w64`
- install any additional native packages required by the project's build
  dependencies
- add the Rust target `x86_64-pc-windows-gnu`
- set environment variables for Cargo to use the MinGW linker and archiver
- build the project for the Windows GNU target

The image should keep the toolchain explicit instead of depending on Cargo's
automatic linker discovery. This reduces ambiguity and makes failures easier to
diagnose.

## Build Strategy

The build should happen inside Docker with the repository mounted or copied into
the build context.

The first iteration should use a multi-stage Dockerfile structure:

### Builder stage

Responsibilities:

- install the Windows cross toolchain
- install the Rust Windows target
- compile `arp-scan-rs` for `x86_64-pc-windows-gnu`

Expected build command:

```bash
cargo build --release --target x86_64-pc-windows-gnu
```

### Artifact stage

The final stage only needs to expose the built executable at a predictable path
inside the image so the outer script can copy it out reliably.

Example internal path:

```text
/out/arp-scan-rs.exe
```

This keeps extraction logic simple and decoupled from Cargo's internal target
directory layout.

## Script Interface

The first iteration script should optimize for stability over flexibility.

Default behavior:

- build release profile
- target `x86_64-pc-windows-gnu`
- export to `dist/windows-x64/arp-scan-rs.exe`

The script should:

- resolve the repository root robustly
- create the output directory if missing
- build the Docker image with a stable tag owned by this repo
- create a temporary container from the image
- copy the executable from the container to the output directory
- clean up the temporary container on success or failure

The script should use `set -euo pipefail`.

The first iteration does not need argument parsing beyond the default workflow.
If optional flags are added later, they should not change the default artifact
path contract.

## Dependency Considerations

This project depends on:

- `slint`
- `slint-build`
- `windows` on Windows targets

The Docker image must include whatever native packages are required for those
dependencies to compile for `x86_64-pc-windows-gnu`.

If a dependency requires additional system libraries for Windows GNU builds,
those libraries should be installed in the image rather than documented as host
prerequisites.

## Error Handling

Failure modes that must be handled cleanly:

- Docker is unavailable.
- Docker image build fails.
- Cargo build fails inside the container.
- The expected executable path is missing from the container.
- The output directory cannot be created or written.

Handling rules:

- the script must stop immediately on failure
- the failing command's output must remain visible
- temporary containers must still be removed
- the script must not silently fall back to a host build

## Verification

Implementation verification for this feature should include:

1. Running the repository build script from the host.
2. Confirming that `dist/windows-x64/arp-scan-rs.exe` is produced.
3. Confirming the script is rerunnable without manual cleanup.
4. Confirming the host still does not require local MinGW or Windows Rust
   target installation.

The first iteration does not require executing the produced `.exe` on Windows.

## Future Extensions

Possible follow-up work, explicitly out of scope for this spec:

- add a debug/profile flag
- add GitHub Actions reuse of the same Dockerfile
- add checksum or zip packaging
- add Wine-based smoke tests
- support additional Windows targets
