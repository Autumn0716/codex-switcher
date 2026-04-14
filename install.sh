#!/usr/bin/env bash
# csw one-click installer
# Usage: curl -fsSL https://raw.githubusercontent.com/Autumn0716/codex-switcher/main/install.sh | bash
# Or:    bash install.sh

set -euo pipefail

GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

info()    { echo -e "${GREEN}==>${NC} $*"; }
warn()    { echo -e "${YELLOW}!!>${NC} $*"; }
fail()    { echo -e "${RED}XX>${NC} $*" >&2; exit 1; }

# ── Check dependencies ───────────────────────────────────────────────────────
command -v python3 &>/dev/null || fail "python3 not found. Install Python 3.10+"
command -v uv &>/dev/null || {
    warn "uv not found — installing uv..."
    curl -LsSf https://astral.sh/uv/install.sh | sh
    export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
}

PYTHON_VER=$(python3 -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")')
PYTHON_MAJOR=$(echo "$PYTHON_VER" | cut -d. -f1)
PYTHON_MINOR=$(echo "$PYTHON_VER" | cut -d. -f2)
if [ "$PYTHON_MAJOR" -lt 3 ] || ([ "$PYTHON_MAJOR" -eq 3 ] && [ "$PYTHON_MINOR" -lt 10 ]); then
    fail "Python 3.10+ required (found $PYTHON_VER)"
fi

# ── Determine repo root ──────────────────────────────────────────────────────
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# If running from curl, clone the repo
if [ "$REPO_DIR" = "." ] || [ ! -f "$REPO_DIR/codex_switcher.py" ]; then
    REPO_DIR=$(mktemp -d)
    info "Cloning codex-switcher..."
    git clone https://github.com/Autumn0716/codex-switcher.git "$REPO_DIR"
fi

# ── Install via uv tool ──────────────────────────────────────────────────────
info "Installing csw via uv tool..."
cd "$REPO_DIR"
uv tool install . --force 2>&1 | tail -3

# ── Verify ────────────────────────────────────────────────────────────────────
if command -v csw &>/dev/null; then
    echo ""
    info "csw installed successfully!"
    info "Run: csw ls"
    echo ""
    csw --help 2>&1 | head -6
else
    fail "Installation failed — csw not found in PATH"
fi
