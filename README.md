# arp-scan-rs

`arp-scan-rs` is a Slint-based desktop UI for ARP host discovery, with a
repository-owned Docker workflow for building a Windows x64 executable from a
Linux host.

## Build Output Location

When the Windows build workflow succeeds, the executable is written to:

```text
dist/windows-x64/arp-scan-rs.exe
```

This is the stable output location for the Docker cross-build workflow. The
`dist/` directory is generated output and is ignored by git, so a fresh checkout
will not contain the `.exe` until you run the build script.

## What This Repository Supports

- Linux host development with `cargo check` and `cargo test`
- Windows-target cross-compilation inside Docker
- Windows x64 executable output at a stable repository path

The actual ARP system probe is Windows-only. On non-Windows hosts, the build
and tests work, but runtime scanning is reported as unsupported.

## Prerequisites

To use the Windows build workflow from Linux, you need:

- Docker installed
- Docker daemon running
- Permission to run `docker build`, `docker create`, `docker cp`, and
  `docker rm -f`

You do not need to install MinGW, a Windows Rust target, or other cross-build
toolchains on the host.

## Build A Windows x64 Executable

Run this from the repository root:

```bash
./scripts/build-windows-gnu.sh
```

What the script does:

1. Builds the Docker image defined in `docker/windows-gnu/Dockerfile`
2. Compiles the project for `x86_64-pc-windows-gnu` in release mode
3. Copies the executable out of the container
4. Writes the result to `dist/windows-x64/arp-scan-rs.exe`

On success, the script prints the final artifact path.

## Artifact Safety

The build wrapper does not delete the previous `dist/windows-x64/arp-scan-rs.exe`
before a new build succeeds.

The replacement flow is:

1. Copy the new executable to a temporary file
2. Move the temporary file into place only after the copy succeeds
3. Clean up the temporary container and temporary file on exit

If a build fails, the last successful `.exe` remains in place.

## Use The Produced `.exe`

After the build completes:

1. Take `dist/windows-x64/arp-scan-rs.exe`
2. Move or copy it to a Windows x64 machine
3. Run it there

The binary is built as a Windows GUI application, so it is intended to be
started directly on Windows rather than used as a Linux executable.

## Local Validation

For host-side validation on Linux:

```bash
cargo check
cargo test -v
```

To rebuild the Windows executable:

```bash
./scripts/build-windows-gnu.sh
file dist/windows-x64/arp-scan-rs.exe
```

Expected artifact type:

```text
PE32+ executable (GUI) x86-64
```

## Relevant Files

- `scripts/build-windows-gnu.sh`
- `docker/windows-gnu/Dockerfile`
- `src/main.rs`
- `src/scan_master/arp_core.rs`
- `tests/windows_link_name.rs`
