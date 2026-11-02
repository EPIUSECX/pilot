# Pilot Desktop

Pilot Desktop is a macOS companion for Frappe developers. It finds your Pilot installation, lists your benches, starts one, opens Pilot's own dashboard in a window, shows startup logs, opens the bench in your editor and stops it. Pilot's Python engine and Admin dashboard are reused as they are.

The shell is Tauri 2 with a Vue 3 and TypeScript launcher built on Frappe UI 1.1.0. The product name and bundle identifier are configurable (see `docs/RELEASE.md`).

## Supported environment

Apple Silicon (arm64) on macOS 14 or later. Intel builds are not configured. A Pilot installation (`~/pilot` by default) with Python 3.11 or later for the `pilot` command, Node.js 24, Git, uv and Redis. Pilot Desktop checks each one and installs what is missing. Running the installed app does not need Rust or Xcode.

## Daily use

1. Open Pilot Desktop. If this Mac is missing anything, a single screen offers Install Pilot. Confirm once and Pilot Desktop adds what is missing (Homebrew, Node.js 24, Pilot with its database, Redis and web server, Python) and shows progress in plain steps, with details one click away. It asks for your Mac password only when Homebrew itself has to be installed. If you already have Pilot, choose I already have Pilot and pick its folder. Diagnostics lists every check.
2. On Benches, choose Start. If the bench is already running, Pilot Desktop attaches to it and does not start a second copy.
3. Choose Open Pilot for the dashboard window. Use Logs for startup output, and the menu for Open in editor, Reveal in Finder, Restart and Reset dashboard session.
4. Closing the window keeps the app in the menu bar. Quit stops the benches this app started and leaves attached benches running.

## Develop

```bash
cd desktop
npm ci
npm run tauri:dev
```

Building from source needs Node.js 20.19 or later, Rust (1.80 or later) and the Xcode command line tools.

## Check

```bash
npm run check:dependencies
npm run lint
npm run typecheck
npm test
cargo test --manifest-path src-tauri/Cargo.toml
uv run --no-project --python 3.12 --with pytest python -m pytest bridge/tests ../tests/admin/backend/test_run_server_host.py
```

## Build

```bash
npm run tauri:build
```

The unsigned `.app` and `.dmg` are written under `src-tauri/target/release/bundle/`. See `docs/RELEASE.md` for signing and notarization.

## Layout

| Path | Contents |
| --- | --- |
| `src/` | Vue launcher: pages, Frappe UI components, composables, typed Tauri adapter |
| `src-tauri/` | Rust native layer: discovery, process ownership, readiness, redaction, commands, tray |
| `bridge/` | Read-only Python adapter over Pilot's inventory and task state |
| `docs/` | Upstream contracts, architecture, validation and release notes |

The upstream change this app needs is `PILOT_ADMIN_HOST` in `admin/backend/run_server.py`, documented in `docs/configuration.md`.
