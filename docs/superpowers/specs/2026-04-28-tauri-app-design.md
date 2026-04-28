# Codex Switcher Tauri App Design

## Goal

Build a cross-platform desktop app for Codex Switcher using Tauri v2. The app manages the same local profile files as the CLI and keeps the overwrite protection behavior: profiles must never be silently replaced by a different `account_id`.

## Assumptions

- Desktop only for the first version: macOS, Windows, and Linux.
- The app name is `Codex Switcher`.
- The frontend is React with TypeScript and Vite.
- The backend is Rust inside `src-tauri`, with no Python runtime dependency.
- Existing storage remains compatible with the CLI:
  - Codex auth: `~/.codex/auth.json`
  - Profiles: `~/.csw/profiles/*.json`
  - Config: `~/.csw/config.json`
- Usage quota fetching is out of scope for the first app version.
- The frontend never reads tokens directly from disk. Rust commands read files and return redacted display data.

## Architecture

The project adds a Tauri app at the repository root. `src-tauri` owns all filesystem operations and exposes a small command API to the React UI:

- `list_profiles`
- `add_profile`
- `switch_profile`
- `rename_profile`
- `remove_profile`
- `get_requirements`

The Rust profile service is written as testable logic that accepts a `CswPaths` struct. Production paths use the user's home directory; tests use temporary directories.

## Data Model

`ProfileInfo` returned to the frontend contains:

- `name`
- `email`
- `plan`
- `account_id_hash`
- `is_active`
- `is_current_auth`
- `id_token_expired`
- `access_token_expired`

Raw token strings and full account ids are not sent to the UI.

## Behavior

List profiles:

- Read `~/.csw/profiles/*.json`, excluding names starting with `__`.
- Decode JWT payloads without verification for display fields.
- Compare profile `account_id` with active config and current auth.

Add profile:

- Validate names with the existing CLI character rules.
- Read current `~/.codex/auth.json`.
- Reject if the same `account_id` is already saved under another name.
- Reject if the requested profile name exists and belongs to a different `account_id`.
- Allow refreshing an existing profile only when `account_id` matches.

Switch profile:

- Before switching, refresh the current active profile only when current auth and saved active profile have the same `account_id`.
- Copy selected profile into `~/.codex/auth.json` atomically.
- Save `config.active`.

Rename and remove:

- Use the same validation rules as the CLI.
- Update `config.active` when needed.

## UI Direction

The app should feel like a compact operator console, not a marketing page. The main screen is a dense account table with a right-side detail/actions panel.

Visual rules:

- Dark neutral base with cyan/green status accents.
- No oversized hero sections.
- No nested cards.
- Rows are stable height and easy to scan.
- Buttons use icon+text for destructive or primary actions.
- Token status uses clear chips: current, active, expired, missing.

## Testing

- Rust unit tests cover add, switch, rename, remove, duplicate detection, same-name overwrite refusal, and active-profile safe refresh.
- TypeScript build validates frontend types.
- Tauri build validates integration when the local platform dependencies are present.

## Requirements For Users

macOS:

- Xcode Command Line Tools.
- Optional Apple Developer account for signed/notarized distribution.

Windows:

- Microsoft C++ Build Tools with Desktop development with C++.
- Microsoft Edge WebView2 Runtime, usually already present on modern Windows.
- MSVC Rust toolchain.

Linux:

- WebKitGTK 4.1 development packages and build tools.

## Out Of Scope

- Usage quota API display.
- Auto-updater.
- System tray.
- Store distribution.
- Import/export encrypted vault.
