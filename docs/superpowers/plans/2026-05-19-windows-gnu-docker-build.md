# Windows GNU Docker Build Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a repository-owned Docker workflow that builds `arp-scan-rs.exe` for `x86_64-pc-windows-gnu` and exports it to `dist/windows-x64/arp-scan-rs.exe` without installing Windows toolchains on the host.

**Architecture:** The repository will own a dedicated Docker builder image plus a small shell wrapper. The Dockerfile encapsulates the Rust Windows GNU target and MinGW toolchain, while the shell script builds the image, extracts the executable from a predictable container path, and leaves the host environment unchanged apart from the generated artifact.

**Tech Stack:** Docker, Rust 1.95, `mingw-w64`, Bash, Cargo cross-compilation for `x86_64-pc-windows-gnu`

---

## File Structure

- Modify: `.gitignore`
  - Ignore generated `dist/` artifacts so Windows executables are not accidentally committed.
- Create: `.dockerignore`
  - Keep Docker build context small and deterministic by excluding `target/`, `dist/`, `.git/`, and docs.
- Create: `docker/windows-gnu/Dockerfile`
  - Define a reproducible Windows GNU builder image and expose `/out/arp-scan-rs.exe` from a final artifact stage.
- Create: `scripts/build-windows-gnu.sh`
  - Build the Docker image, create a temporary container, copy out the executable, and clean up.

### Task 1: Define Output Hygiene And Docker Build Context

**Files:**
- Modify: `.gitignore`
- Create: `.dockerignore`

- [ ] **Step 1: Add `dist/` to `.gitignore`**

Update `.gitignore` from:

```gitignore
/target
```

to:

```gitignore
/target
/dist
```

- [ ] **Step 2: Create `.dockerignore`**

Create `.dockerignore` with:

```dockerignore
.git
target
dist
docs
```

- [ ] **Step 3: Verify the new ignore rules**

Run:

```bash
mkdir -p dist/windows-x64
touch dist/windows-x64/arp-scan-rs.exe
git check-ignore -v dist/windows-x64/arp-scan-rs.exe
```

Expected:

- output references `.gitignore`
- the path `dist/windows-x64/arp-scan-rs.exe` is reported as ignored

- [ ] **Step 4: Verify Docker context exclusions are present**

Run:

```bash
sed -n '1,120p' .dockerignore
```

Expected:

- output contains exactly:
  - `.git`
  - `target`
  - `dist`
  - `docs`

- [ ] **Step 5: Commit**

```bash
git add .gitignore .dockerignore
git commit -m "chore: define windows build output hygiene"
```

### Task 2: Add The Windows GNU Docker Builder Image

**Files:**
- Create: `docker/windows-gnu/Dockerfile`

- [ ] **Step 1: Create the Dockerfile**

Create `docker/windows-gnu/Dockerfile` with:

```dockerfile
FROM rust:1.95-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        mingw-w64 \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-pc-windows-gnu

ENV CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
ENV CARGO_TARGET_X86_64_PC_WINDOWS_GNU_AR=x86_64-w64-mingw32-gcc-ar
ENV CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc
ENV CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++

WORKDIR /app
COPY . .

RUN cargo build --release --target x86_64-pc-windows-gnu
RUN install -D target/x86_64-pc-windows-gnu/release/arp-scan-rs.exe /out/arp-scan-rs.exe

FROM debian:bookworm-slim AS artifact

COPY --from=builder /out/arp-scan-rs.exe /out/arp-scan-rs.exe
```

- [ ] **Step 2: Build the Docker image**

Run:

```bash
docker build \
  -f docker/windows-gnu/Dockerfile \
  -t arp-scan-rs/windows-gnu:test \
  .
```

Expected:

- Docker build completes successfully
- the final image tag `arp-scan-rs/windows-gnu:test` exists locally

- [ ] **Step 3: Verify the artifact path inside the image**

Run:

```bash
docker run --rm --entrypoint ls arp-scan-rs/windows-gnu:test /out/arp-scan-rs.exe
```

Expected:

- command prints `/out/arp-scan-rs.exe`

- [ ] **Step 4: Commit**

```bash
git add docker/windows-gnu/Dockerfile
git commit -m "build: add windows gnu docker builder"
```

### Task 3: Add The Host Wrapper Script And Verify End-To-End Export

**Files:**
- Create: `scripts/build-windows-gnu.sh`

- [ ] **Step 1: Create the build wrapper script**

Create `scripts/build-windows-gnu.sh` with:

```bash
#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image_tag="arp-scan-rs/windows-gnu:latest"
artifact_dir="$repo_root/dist/windows-x64"
artifact_path="$artifact_dir/arp-scan-rs.exe"
container_id=""

cleanup() {
    if [[ -n "$container_id" ]]; then
        docker rm -f "$container_id" >/dev/null 2>&1 || true
    fi
}

trap cleanup EXIT

mkdir -p "$artifact_dir"
rm -f "$artifact_path"

docker build \
    -f "$repo_root/docker/windows-gnu/Dockerfile" \
    -t "$image_tag" \
    "$repo_root"

container_id="$(docker create "$image_tag")"
docker cp "$container_id:/out/arp-scan-rs.exe" "$artifact_path"

echo "Built Windows executable: $artifact_path"
```

- [ ] **Step 2: Mark the script executable**

Run:

```bash
chmod +x scripts/build-windows-gnu.sh
```

Expected:

- script becomes executable in git mode changes

- [ ] **Step 3: Verify shell syntax**

Run:

```bash
bash -n scripts/build-windows-gnu.sh
```

Expected:

- no output
- exit code `0`

- [ ] **Step 4: Run the end-to-end Docker build**

Run:

```bash
./scripts/build-windows-gnu.sh
```

Expected:

- Docker builds or reuses the image
- `dist/windows-x64/arp-scan-rs.exe` is created
- the script prints the final artifact path

- [ ] **Step 5: Verify the exported executable exists**

Run:

```bash
ls -l dist/windows-x64/arp-scan-rs.exe
file dist/windows-x64/arp-scan-rs.exe
```

Expected:

- `ls` shows a non-zero-size file
- `file` reports a `PE32+ executable` or equivalent Windows 64-bit executable description

- [ ] **Step 6: Verify rerun behavior**

Run:

```bash
./scripts/build-windows-gnu.sh
```

Expected:

- command succeeds again without manual cleanup
- output artifact is overwritten cleanly

- [ ] **Step 7: Commit**

```bash
git add scripts/build-windows-gnu.sh
git commit -m "feat: add windows gnu docker build wrapper"
```

## Self-Review

### Spec Coverage

- Docker-owned toolchain isolation is covered by Task 2.
- Stable repository script entrypoint is covered by Task 3.
- Stable artifact path `dist/windows-x64/arp-scan-rs.exe` is covered by Tasks 1 and 3.
- Failure visibility and host cleanliness are covered by Task 3's shell behavior and Task 2's containerized build.

### Placeholder Scan

- No `TODO`, `TBD`, or unresolved placeholders remain.
- All new file paths are explicit.
- All verification commands are concrete and runnable.

### Type And Interface Consistency

- The Dockerfile exports `/out/arp-scan-rs.exe`.
- The shell script copies from `/out/arp-scan-rs.exe`.
- The repository artifact path is consistently `dist/windows-x64/arp-scan-rs.exe`.
