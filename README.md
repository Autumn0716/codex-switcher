# Code Switcher

Desktop application for managing multiple AI coding provider accounts. Switch between Claude Code, Codex, Gemini, OpenClaw, or any OpenAI-compatible endpoint — without manually editing config files.

## Core Capabilities

### Profile Management

Save provider JSON files for each supported coding tool, then select or activate them from the desktop UI.

- **Add Profile** — Opens the edit-config modal first; `Confirm` creates one provider card and one provider JSON file
- **Cancel / Confirm** — Cancel leaves no provider behind; confirm saves under `~/.csw/config/<brand>/<provider>.json`
- **Edit Config** — Updates the provider fields and generated config content
- **Save** — Activates the selected provider for that brand

Each card represents exactly one provider JSON file.

### Custom Provider Editor

Connect any OpenAI-compatible API endpoint as a first-class provider.

- Enter the API base URL, model name, and API key
- The app generates a valid `auth.json` and `config.toml` for the target tool
- Custom file paths can be overridden if your tool lives in a non-standard location
- API keys are never displayed in full — previews show only the first and last 4 characters
- Existing files are backed up with timestamps before being overwritten

### Provider Dashboard

The desktop UI provides an at-a-glance view of each provider's configuration:

- Active model name and version
- Codex account email, plan, 5-hour usage window, weekly usage window, and reset timing when `auth.json` is available
- Token limits, temperature, and streaming settings
- Connection health and uptime status
- Profile count and validation state

### Safety Guarantees

- **Atomic Writes** — All file writes use a temp-then-rename strategy to prevent corruption
- **Automatic Backups** — Every overwritten auth or config file is backed up with a `.bak.{timestamp}` extension
- **Permission Lockdown** — Auth files are written with `0600` permissions on Unix systems
- **JWT Expiry Detection** — Token expiration is parsed from JWT claims and displayed in the UI
- **Account ID Hashing** — Profiles display a SHA-256 hash of the account ID, never the raw credential

## Architecture

| Layer | Technology |
|-------|------------|
| Desktop Shell | Tauri v2 (Rust backend + React frontend) |
| Frontend | React 18, TypeScript, Framer Motion, Phosphor Icons, Tailwind CSS v4 |
| Backend | Rust with `serde`, `serde_json`, `toml`, `sha2`, `base64` |
| IPC | Tauri commands exposed from Rust to the React UI |
| Storage | Local filesystem (`~/.csw/`, `~/.codex/`, `~/.code-switcher/`) |

### File Layout

```
~/.csw/
├── config/
│   ├── claude/            # Provider JSON, e.g. anthropic.json, deepseek.json
│   ├── codex/             # Provider JSON, e.g. openai.json, qwen.json
│   └── gemini/            # Provider JSON, e.g. google.json
├── prompts/               # Prompt documents
├── diagrams/              # Architecture and design documents
├── data/logs.db           # Request logs and usage summaries
├── backups/               # Backups
├── profiles/              # Saved Codex auth snapshots
└── config.json            # Active Codex CLI profile tracker

~/.codex/
└── auth.json           # Currently active credentials

~/.code-switcher/
├── custom-providers.json  # Registry of custom providers
└── providers/             # Per-provider auth.json + config.toml
```

## Getting Started

### Prerequisites

- **Rust toolchain** — `rustup default stable`
- **Node.js** — v20 or later; Linux packaging uses Node 22

### Development

```bash
npm install
npm run app:dev
```

`npm run app:dev` is the normal beta/dev app entrypoint. It calls `npm run tauri:dev` underneath, which starts the Tauri desktop shell and Vite dev server. Frontend changes hot-reload into the running app. Rust backend changes require the Tauri dev process to rebuild or restart.

Use plain Vite only for browser UI checks:

```bash
npm run dev
```

Plain browser mode cannot call Tauri commands, so it does not verify local file persistence.

### Local File Testing

Release builds can read and write the app-managed local files required for normal operation, such as `~/.csw/...`, `~/.codex/auth.json`, and legacy `~/.code-switcher/...` paths. The app should not expose a release command that reads arbitrary user-selected filesystem paths without a narrow product requirement. That restriction is for data-leak prevention, not because release builds cannot access local files.

Recommended persistence test flow:

```bash
npm run app:dev
```

Then:

1. Edit a provider profile in the desktop app.
2. Save or activate the profile.
3. Check the matching JSON file under `~/.csw/config/<brand>/`.
4. Restart `npm run app:dev`.
5. Confirm the profile loads from disk.

Codex auth test flow:

1. Open the Codex provider edit view.
2. Click `Codex Login`; the app starts `codex login` in the background, without opening Terminal.
3. The app opens the OpenAI auth page returned by the Codex CLI.
4. Codex writes `~/.codex/auth.json` after login succeeds.
5. The app waits for `~/.codex/auth.json` to change and imports it into the current Codex provider.
6. If `~/.codex/auth.json` already exists, click `Import auth.json` to import it directly.
7. Return to the provider cards and confirm the Codex card shows account and usage information, or an explicit auth/usage error state.

Default Codex config generated by the app:

```toml
[codex]
model = "gpt-5.5"
model_provider = "openai"
model_context_window = 1000000
model_auto_compact_token_limit = 900000
model_reasoning_effort = "xhigh"
approvals_reviewer = "user"
```

### Build a Desktop App

macOS:

```bash
npm run tauri:build        # .app bundle
npm run tauri:build:dmg    # .dmg installer
```

`npm run tauri:build` writes the release app to `src-tauri/target/release/bundle/macos/code-switcher.app`. Cargo build output is kept under `src-tauri/target`.

Linux packages must be built in a Linux environment. From macOS, use the Docker wrapper:

```bash
npm run linux:build:docker
```

The Docker build writes `.deb`, `.rpm`, and `.AppImage` files to `target/linux-release/`. Inside Linux, or in CI, run `npm run linux:build`; Tauri writes bundles under `src-tauri/target/release/bundle/`.

GitHub Actions can also build Linux test artifacts through the `Linux Build` workflow. Run it manually with `workflow_dispatch`, or push a `v*` tag. The workflow uploads a `code-switcher-linux` artifact containing `.deb`, `.rpm`, and `.AppImage` files.

Linux release builds use the same app-managed local paths as macOS:

```text
~/.csw/
~/.codex/
~/.claude/
~/.gemini/
```

Windows: Install Microsoft C++ Build Tools and the Rust MSVC toolchain, then run `npm run tauri:build`.

## CLI

The `csw` command provides terminal-based profile management.

```bash
curl -fsSL https://raw.githubusercontent.com/Autumn0716/code-switcher/main/install.sh | bash
```

```bash
csw add <name>        # Save current account as a profile
csw ls                # List all profiles and switch interactively
csw switch <name>     # Switch to a named profile
csw current           # Show which profile is active
csw balance           # Check token usage for current account
csw rm <name>         # Delete a profile
csw mv <old> <new>    # Rename a profile
```

## License

MIT
