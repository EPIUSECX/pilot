# Implementation status

Last updated: 2026-11-01. Target: Apple Silicon (arm64), macOS 27 dev machine (Darwin 27.0.0, arm64). Product name and bundle identifier are configurable (`docs/RELEASE.md`).

## Completed stages

1. Foundation. Pilot source, docs and CLI help inspected. Contracts, versions and the verification method are in `docs/UPSTREAM.md` (Pilot `ee57c4b3`, `frappe-ui` 1.1.0, Tailwind 3.4.19, Vue 3.5.44, Vite 8.3.5, Tauri 2.12.x).
2. First working path. Tauri app, Frappe UI v1 bench selector, environment detection, attach-or-start, readiness by verified identity, dashboard window per bench with its own WebKit data store.
3. Everyday operation. One-step in-app setup (Apple developer tools, Homebrew, Node.js 24, Pilot, Python; progress, retry, cancel, plain-language failures), bench creation, attach detection, sanitized logs, safe stop and restart, editor and Finder shortcuts, preferences, menu bar, quit flow.
4. Validation and packaging. Automated suites pass (see `docs/VALIDATION.md`). An unsigned arm64 `.app` and `.dmg` were built on this Mac.

Upstream change made: `PILOT_ADMIN_HOST` in `admin/backend/run_server.py` (default unchanged), with `tests/admin/backend/test_run_server_host.py` and a note in `docs/configuration.md`.

## Not verified (needs a person at a Mac)

- Visible window, tray icon and menu content of the built app. This session had no Screen Recording or Accessibility permission, so only "the bundle launches through LaunchServices and stays running" was observed.
- WKWebView behavior of the Pilot dashboard (sign-in, navigation, task updates, uploads, downloads), per-bench cookie isolation, and keyboard focus in native dialogs. The Vue launcher was only checked in a browser at 760x560 with preview-only mock data.
- An end-to-end run against a real Pilot bench: `~/pilot` on this Mac has no `bin/pilot`, Node is 22 (Pilot needs 24), Redis is not installed, so create, start, wizard and stop were exercised against a fake Pilot, not Frappe.
- Quit dialog with an owned running bench, menu-bar behavior after closing the window, second-instance launch.
- Styled DMG window (Finder scripting is not permitted here, so the DMG is built plain with `CI=true`).
- Intel architecture, Developer ID signing, notarization.

## Next concrete task

On a Mac with Pilot installed (Node 24, Redis, Python 3.11+): run `docs/VALIDATION.md` section "Manual macOS checklist" against a disposable bench, fix what it finds, then add the Developer ID secrets and run `.github/workflows/desktop-release.yml`.

## Known limits

- A downloaded unsigned build is not notarized, so macOS asks the user to confirm on first open. `desktop-v0.1.0` shipped with a partial signature that macOS called damaged. `desktop-v0.2.0` carries a sealed ad-hoc signature, verified through the zip and DMG round trips and after a fresh download.

- Production benches and benches with the Admin disabled are listed but cannot be started from Desktop.
- Sleep and resume are handled by a 10 second re-probe, not an NSWorkspace notification.
- The Pilot installer script is downloaded from a branch URL, so its content can change. Its SHA-256 is logged and recorded. The user confirms once in the app before anything is installed.
- The in-app setup is unit-tested with a scripted executor and real process tests, not against a Mac without Homebrew. The Homebrew branch (native password dialog through `SUDO_ASKPASS`, and Homebrew's own handling of it) and the Apple developer tools wait are unverified. Homebrew's installer is documented to prefer `SUDO_ASKPASS` over its non-interactive `sudo -n` check; confirm on a clean Mac.
- `cargo test` prints a harmless linker warning about a duplicate `__EMBED_INFO_PLIST` symbol in the IPC authorization test.
- `npm audit` reports 3 moderate advisories (PostCSS selector parser, reached through `frappe-ui`'s `@tailwindcss/typography` at build time). The only offered fix downgrades `frappe-ui` to 0.1.109, which breaks the v1 requirement, so it is not applied. Revisit when `frappe-ui` 1.x updates that dependency.
- Pilot builds without `PILOT_ADMIN_HOST` (the fix lives in this repository's `admin/backend/run_server.py`) bind the Admin to every network interface. Desktop detects this on a running bench and shows a note. Found on a real install on 2 November 2026.
- Pilot's PostgreSQL support on macOS expects its own server on port 5450 while Homebrew's runs on 5432, so a bench set to PostgreSQL stops at "Install system packages" with "PostgreSQL did not become reachable within 30s". This is in Pilot, not Desktop. Choose MariaDB for macOS benches until Pilot fixes it.
- Real-bench findings that fakes could not show: the Admin API lives under `/api/v1` (fixed in `readiness.rs`, with a regression test and fixtures that now answer 200 with a page for every other path).
