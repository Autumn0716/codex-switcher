# Linux Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a repeatable Linux packaging path for the Tauri desktop app that produces `.deb`, `.rpm`, and `.AppImage` artifacts from a Linux build environment.

**Architecture:** Keep macOS development unchanged. Add Linux-specific npm scripts for commands that run inside Linux, a Docker-backed local build wrapper for macOS users, and a GitHub Actions workflow that builds Linux artifacts on Ubuntu and uploads them for testing. Document that the app continues to read/write `~/.csw`, `~/.codex`, `~/.claude`, and `~/.gemini` on Linux.

**Tech Stack:** Tauri v2, Rust, npm, Vite, Docker, GitHub Actions Ubuntu runner.

---

## Assumptions

- The current macOS machine cannot directly produce Linux Tauri bundles; Linux bundles must be built in Linux.
- Primary output formats are `.deb`, `.rpm`, and `.AppImage`.
- The initial Linux pipeline is for test artifacts, not signed/notarized production release automation.
- No arbitrary local filesystem access is added; release builds only use the app-managed config paths already supported by the backend.

## File Structure

- Modify `package.json`: add Linux build scripts without changing existing macOS scripts.
- Create `packaging/linux/Dockerfile`: define a Linux build image with system dependencies needed by Tauri.
- Create `packaging/linux/build-linux.sh`: build Linux bundles inside the container and copy artifacts to a predictable output directory.
- Create `.github/workflows/linux-build.yml`: build and upload `.deb`, `.rpm`, and `.AppImage` artifacts on Ubuntu.
- Modify `README.md`: document local Docker build, GitHub Actions build, output paths, and Linux config paths.
- Modify `todolist.md`: record what was implemented and remaining limitations.

## Task 1: Linux npm Scripts

**Files:**
- Modify: `package.json`

- [x] **Step 1: Add package scripts**

Add these scripts under `scripts`:

```json
"linux:build": "tauri build --bundles deb,rpm,appimage",
"linux:build:docker": "docker build -f packaging/linux/Dockerfile -t code-switcher-linux-builder . && docker run --rm -v \"$(pwd):/workspace\" code-switcher-linux-builder"
```

- [x] **Step 2: Verify package JSON**

Run:

```bash
node -e "JSON.parse(require('fs').readFileSync('package.json', 'utf8')); console.log('package.json ok')"
```

Expected:

```text
package.json ok
```

## Task 2: Docker Linux Builder

**Files:**
- Create: `packaging/linux/Dockerfile`
- Create: `packaging/linux/build-linux.sh`

- [x] **Step 1: Create Dockerfile**

Use Ubuntu and install the Tauri Linux build dependencies:

```dockerfile
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
WORKDIR /workspace

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    curl \
    file \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libssl-dev \
    libwebkit2gtk-4.1-dev \
    libxdo-dev \
    patchelf \
    rpm \
    wget \
    xdg-utils \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get update \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

ENV PATH="/root/.cargo/bin:${PATH}"

COPY packaging/linux/build-linux.sh /usr/local/bin/build-linux
RUN chmod +x /usr/local/bin/build-linux

CMD ["/usr/local/bin/build-linux"]
```

- [x] **Step 2: Create build script**

```bash
#!/usr/bin/env bash
set -euo pipefail

cd /workspace

npm ci
npm run linux:build

mkdir -p /workspace/target/linux-release
find /workspace/src-tauri/target/release/bundle -type f \
  \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) \
  -exec cp {} /workspace/target/linux-release/ \;

printf '\nLinux artifacts:\n'
ls -lh /workspace/target/linux-release
```

- [x] **Step 3: Verify script syntax**

Run:

```bash
bash -n packaging/linux/build-linux.sh
```

Expected: no output and exit code `0`.

## Task 3: GitHub Actions Linux Artifacts

**Files:**
- Create: `.github/workflows/linux-build.yml`

- [x] **Step 1: Add workflow**

```yaml
name: Linux Build

on:
  workflow_dispatch:
  push:
    tags:
      - "v*"

jobs:
  linux:
    name: Build Linux bundles
    runs-on: ubuntu-22.04

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: "22"
          cache: npm

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install Linux dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y --no-install-recommends \
            build-essential \
            curl \
            file \
            libayatana-appindicator3-dev \
            librsvg2-dev \
            libssl-dev \
            libwebkit2gtk-4.1-dev \
            libxdo-dev \
            patchelf \
            rpm \
            wget

      - name: Install frontend dependencies
        run: npm ci

      - name: Build Linux bundles
        run: npm run linux:build

      - name: Upload Linux bundles
        uses: actions/upload-artifact@v4
        with:
          name: code-switcher-linux
          path: |
            src-tauri/target/release/bundle/deb/*.deb
            src-tauri/target/release/bundle/rpm/*.rpm
            src-tauri/target/release/bundle/appimage/*.AppImage
          if-no-files-found: error
```

- [x] **Step 2: Verify workflow YAML parses**

Run:

```bash
ruby -e "require 'yaml'; YAML.load_file('.github/workflows/linux-build.yml'); puts 'workflow yaml ok'"
```

Expected:

```text
workflow yaml ok
```

## Task 4: README and todolist

**Files:**
- Modify: `README.md`
- Modify: `todolist.md`

- [x] **Step 1: Document Linux build commands**

Add a Linux section that states:

```text
Linux packages are built on Linux. From macOS, use npm run linux:build:docker. In CI, run the Linux Build workflow. Artifacts are .deb, .rpm, and .AppImage under src-tauri/target/release/bundle or target/linux-release for Docker.
```

- [x] **Step 2: Document runtime local paths**

Document:

```text
Linux release builds read and write ~/.csw, ~/.codex, ~/.claude, and ~/.gemini using the same app-managed paths as macOS.
```

- [x] **Step 3: Update todolist**

Record completed Linux packaging work and limitations:

```text
- Linux packages must be produced in Linux, not directly on macOS.
- Docker and GitHub Actions are provided for .deb/.rpm/.AppImage.
- Signing and auto-update are not included yet.
```

## Task 5: Verification

**Files:**
- Read-only verification across changed files.

- [x] **Step 1: Validate JSON and shell syntax**

Run:

```bash
node -e "JSON.parse(require('fs').readFileSync('package.json', 'utf8')); console.log('package.json ok')"
bash -n packaging/linux/build-linux.sh
ruby -e "require 'yaml'; YAML.load_file('.github/workflows/linux-build.yml'); puts 'workflow yaml ok'"
```

Expected:

```text
package.json ok
workflow yaml ok
```

- [x] **Step 2: Validate app build still works locally**

Run:

```bash
npm run build
```

Expected: TypeScript and Vite build complete successfully.

- [x] **Step 3: Validate diff whitespace**

Run:

```bash
git diff --check -- package.json README.md todolist.md packaging/linux/Dockerfile packaging/linux/build-linux.sh .github/workflows/linux-build.yml docs/superpowers/plans/2026-04-29-linux-packaging.md
```

Expected: no output and exit code `0`.

## Execution Notes

- Task 1, Task 2, and Task 3 are independent and can run in parallel if each worker owns only its listed files.
- Task 4 should run after the exact script/workflow names are known.
- Task 5 runs after all file changes are integrated.

## Execution Result

- Used OrbStack Ubuntu amd64 to perform a real Linux package build.
- Installed Node 22 because `@tailwindcss/oxide@4.2.4` requires Node 20+ and Ubuntu's default Node 18 failed the native binding load.
- Added explicit Tauri `bundle.icon` entries so AppImage can find a square icon.
- Generated and copied artifacts to `target/linux-release/`:
  - `code-switcher_0.1.0_amd64.deb`
  - `code-switcher-0.1.0-1.x86_64.rpm`
  - `code-switcher_0.1.0_amd64.AppImage`
- Verification passed: JSON parse, shell syntax, workflow YAML parse, `npm run build`, `cargo fmt --check`, and `git diff --check`.
