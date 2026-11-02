# Implementation status

Last updated: 2026-11-01. Target: Apple Silicon (arm64), macOS 27 dev machine (Darwin 27.0.0, arm64). Product name and bundle identifier are configurable (`docs/RELEASE.md`).

## Completed stages

1. Foundation. Pilot source, docs and CLI help inspected. Contracts, versions and the verification method are in `docs/UPSTREAM.md` (Pilot `ee57c4b3`, `frappe-ui` 1.1.0, Tailwind 3.4.19, Vue 3.5.44, Vite 8.3.5, Tauri 2.12.x).
2. First working path. Tauri app, Frappe UI v1 bench selector, environment detection, attach-or-start, readiness by verified identity, dashboard window per bench with its own WebKit data store.
3. Everyday operation. Guided install handoff to Terminal, bench creation, attach detection, sanitized logs, safe stop and restart, editor and Finder shortcuts, preferences, menu bar, quit flow.
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

- Production benches and benches with the Admin disabled are listed but cannot be started from Desktop.
- Sleep and resume are handled by a 10 second re-probe, not an NSWorkspace notification.
- The guided installer downloads from a branch URL, so its content can change. The user reviews it and confirms in Terminal.
- `cargo test` prints a harmless linker warning about a duplicate `__EMBED_INFO_PLIST` symbol in the IPC authorization test.
- `npm audit` reports 3 moderate advisories (PostCSS selector parser, reached through `frappe-ui`'s `@tailwindcss/typography` at build time). The only offered fix downgrades `frappe-ui` to 0.1.109, which breaks the v1 requirement, so it is not applied. Revisit when `frappe-ui` 1.x updates that dependency.
