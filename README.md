# csw — Codex Switcher

Lightweight CLI to switch between multiple OpenAI Codex accounts. Zero dependencies, Python stdlib only.

## Features

- **Instant account switching** — backs up & replaces `~/.codex/auth.json` atomically
- **Interactive selector** — arrow-key list with real-time usage display
- **Usage quota display** — shows 5h / weekly remaining % with reset times via `/backend-api/wham/usage`
- **File cache** — usage data cached to disk (5 min TTL), no repeated API calls
- **Token expiry check** — warns if access_token is expired before switching
- **Process detection** — warns if Codex CLI is currently running
- **Duplicate detection** — prevents saving the same account twice

## Install

### One-click install

```bash
curl -fsSL https://raw.githubusercontent.com/your-username/codex-switcher/main/install.sh | bash
```

### Manual install

```bash
git clone https://github.com/your-username/codex-switcher.git
cd codex-switcher
uv tool install .
```

Requires: Python 3.10+, `uv` package manager.

## Usage

```
csw add <name>        # Save current account as a profile
csw ls                # Interactive list (arrow keys) with usage info
csw switch <name>     # Switch to a profile
csw current           # Show active profile
csw balance           # Check usage/balance for current account
csw rm <name>         # Remove a profile
csw mv <old> <new>    # Rename a profile
```

## How it works

- Profiles stored in `~/.csw/profiles/` as copies of `auth.json`
- Active profile tracked in `~/.csw/config.json`
- Usage data cached in `~/.csw/cache/<name>.json` (5 min TTL)
- File permissions set to `0o600` for all sensitive files
- Atomic writes via `temp + os.rename()`

## License

MIT
