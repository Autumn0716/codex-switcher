# Code Switcher App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename the desktop app to `code-switcher` and add a multi-vendor UI with a custom OpenAI-compatible provider editor.

**Architecture:** Keep the existing Codex profile backend intact. Add a small Rust-backed custom provider store that writes app-managed provider JSON plus generated `auth.json` and `config.toml` files. The React UI becomes a vendor dashboard with fixed vendor navigation, overview cards, Codex profile actions, and a Custom provider form.

**Tech Stack:** Tauri v2, Rust, React, TypeScript, CSS, lucide-react.

---

### Task 1: Custom Provider Backend

**Files:**
- Modify: `src-tauri/src/profiles.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] Add Rust unit tests for saving a custom provider and generating both `auth.json` and `config.toml`.
- [ ] Implement `list_custom_providers`, `save_custom_provider`, and `remove_custom_provider`.
- [ ] Register the new commands in the Tauri invoke handler.

### Task 2: Multi-Vendor Frontend

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.css`

- [ ] Replace the single Codex layout with left-side vendor navigation.
- [ ] Add a Home dashboard with external account status/quota cards.
- [ ] Keep Codex profile switching actions available.
- [ ] Add a Custom provider editor matching the supplied reference structure.

### Task 3: Project Rename And Packaging

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/src/lib.rs`
- Modify: `README.md`
- Modify: `index.html`

- [ ] Rename package, bundle, app title, and identifiers to `code-switcher`.
- [ ] Build the frontend and Tauri bundle.
- [ ] Rename the local folder to `/Users/jiangxun/code-switcher`.
- [ ] Attempt to rename the GitHub repository to `code-switcher` if local GitHub auth permits it.
