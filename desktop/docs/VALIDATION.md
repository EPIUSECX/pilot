# Validation

Environment: Darwin 27.0.0 (macOS 27.0, build 26A428), arm64 (Apple Silicon), 1 November 2026. Node.js 22.23.1, npm 10.9.8, Rust 1.99.1 (Homebrew, `aarch64-apple-darwin`), Python 3.12.13 (uv) for tests, Command Line Tools only (no full Xcode). Only arm64 was tested.

## Automated checks (all run on this Mac)

| Check | Command | Result |
| --- | --- | --- |
| Frappe UI v1 and peers | `npm run check:dependencies` (also runs on `npm install`) | frappe-ui 1.1.0, tailwindcss 3.4.19, vue 3.5.44 accepted. Unit tests reject 0.x, 2.x, a 1.x prerelease, Tailwind 3.3 and 4, Vue 3.4 and unpinned ranges. |
| Lint | `npm run lint` | clean |
| Types | `npm run typecheck` | clean |
| Frontend tests | `npm test` | 12 passed (dependency check, bounded and searchable logs, listener disposal) |
| Production bundle | `npm run build` | built |
| Rust unit and integration | `cargo test --manifest-path src-tauri/Cargo.toml` | 92 passed (75 library, 12 lifecycle, 4 capability, 1 IPC authorization); the earlier 61 passed 5 of 5 full runs before the setup engine was added |
| Bridge and upstream fix | `python -m pytest desktop/bridge/tests tests/admin/backend/test_run_server_host.py` | 16 passed |
| Python lint | `ruff check` and `ruff format --check` on the changed Python | clean |

## Required risk areas and where they are tested

| Area | Test |
| --- | --- |
| 1. Frappe UI major 1 and peers | `scripts/check-dependencies.test.mjs`; CI step `check:dependencies` |
| 2. Paths with spaces | `lifecycle.rs` (root, benches and bench folders contain spaces); `discovery.rs`, `adapter.rs`, bridge tests |
| 3. No shell injection | `adapter.rs::user_input_is_an_argument_never_shell_text`; `lifecycle.rs` create validation; `setup/executor.rs` (job environment and arguments are explicit, no shell) |
| 4. Attach, no duplicate launch | `start_attaches_to_a_running_bench_without_a_duplicate_launch` (launch count stays 1) |
| 5. Unrelated service rejected | `readiness.rs` classification tests; `an_unrelated_service_on_the_port_is_a_conflict_and_nothing_is_launched` (the impostor answers `/health` and `/bootstrap` with the right name) |
| 6. Timeout, failure, subscriptions | `a_start_that_never_listens_times_out_cleans_up_and_can_be_retried`, `a_crash_during_startup_...`, `subscription.test.ts` |
| 7. Quit and stop scope | `quit_cleanup_stops_owned_benches_and_spares_attached_and_unrelated_ones`, `stopping_an_attached_bench_needs_confirmation_and_leaves_others_running` |
| 8. Reused PID | `process.rs::a_reused_pid_does_not_match_the_recorded_identity` |
| 9. Redaction before UI and files | `redact.rs`, `logs.rs` (buffer, listener and file), `lifecycle.rs` setup-link and password tests |
| 10. Dashboard cannot call native commands | `ipc_authorization.rs` (Tauri mock runtime with the real generated ACL), `access.rs`, `capabilities.rs` |
| 11. Preferences survive updates | `preferences.rs::an_older_file_keeps_its_values_and_gains_defaults` (bench files untouched) |

Also covered: active work blocks Stop and Restart, forced stop reports what it killed, concurrent operations are refused, a stale pid file is ignored, setup links are accepted only for this bench's loopback port, dashboard navigation and download rules, per-bench data store identifiers.

## What these tests use

The lifecycle tests drive real processes: the Rust manager spawns a stand-in `bin/pilot` (a Python script in a folder with spaces) that listens on a loopback port, answers `/health` and `/bootstrap`, writes `pids/bench.pid`, spawns a child and handles `SIGTERM`. It is not Frappe or Pilot. Real Pilot was not started, because this Mac has no usable install (see below).

## Test findings worth knowing

- A real bug was found and fixed: identity matched the executable path, which `/usr/bin/python3` (a macOS shim) replaces by exec, so a started bench could look "attached" and its Stop asked for confirmation. Identity now matches the argument string Desktop passed. A unit test covers a re-exec.
- After an owned stop, Desktop waits for the admin port to be released before reporting the bench stopped.
- Lifecycle tests share the process table and loopback ports, so they now run one at a time (`SERIAL` lock in `tests/common.rs`). Before that, about 1 in 13 parallel runs failed with a spurious port conflict. I did not pin down the exact cause of that parallel-only interference. After serialization: 80 of 80 runs of the lifecycle suite, then 5 of 5 full-suite runs, passed.

## Built artifacts (unsigned, arm64)

`desktop/src-tauri/target/release/bundle/macos/Pilot Desktop.app` (7.63 MiB, Mach-O arm64 thin, ad-hoc signature, `LSMinimumSystemVersion` 14.0, identifier `com.epiuse.pilot-desktop`, bridge script bundled) and `.../dmg/Pilot Desktop_0.1.0_aarch64.dmg` (4.40 MiB, `hdiutil verify` valid, SHA-256 `13fd9d4070b7bf202e0f6fc509a6559cddb1814a5bc633f63af7784e20949d0d`). The DMG is plain: the default styling step drives Finder with AppleScript, which timed out in this session, so it was built with `CI=true`.

Checks on the bundle: it launched through LaunchServices (`open -n`), stayed running with about 100 MB of memory, and exited on `SIGTERM`. The bundled bridge ran against this repository as a Pilot root and returned `{"schema_version": 1, "pilot_version": "dev", ...}`.

## Not verified here

- The visible window, tray icon, menu content and any pixels of the native app. No Screen Recording or Accessibility permission was available.
- WKWebView behavior: dashboard sign-in, navigation, task updates, uploads, downloads, per-bench cookie isolation, keyboard focus and Escape in native dialogs. The launcher was only checked in a browser at 760 by 560 using preview-only mock data (`npm run preview`, never bundled). That showed layout, create-dialog validation and the attached-bench stop confirmation.
- A real Pilot run: `~/pilot` has no `bin/pilot`, Node is 22 (Pilot needs 24) and Redis is missing, so create, start, wizard, sign-in and stop against Frappe did not run.
- Finder-launch specifically (LaunchServices launch was tested), second-instance behavior, window close and reopen with the tray, the quit dialog with an owned running bench, sleep and resume, Open in Cursor (Cursor.app exists on this Mac, the command was not exercised).
- Intel, Developer ID signing, notarization, the styled DMG, and both GitHub workflows (not run).

## Manual macOS checklist

Use a disposable bench on a Mac with Pilot installed (Node 24, Redis, Python 3.11+).

1. Open the DMG, drag the app out, launch it from Finder with no Terminal open. On a Mac that is ready it opens on Benches. On a Mac that is not, it shows one Install Pilot screen: confirm, watch the steps finish, and land on Benches with no further action. Check that the Homebrew path (a Mac without Homebrew) shows the macOS password dialog and that Cancel in it ends with a plain explanation. Diagnostics lists the real results.
2. Create a bench named `desktop-smoke`. Save the password dialog, complete Pilot's setup in the dashboard window, create a test site.
3. In the dashboard window: sign out and in, navigate, run a task and watch its progress, upload and download a backup file.
4. Close the main window. The tray icon stays. Click Open Pilot Desktop in the tray menu, and click the Dock icon: the window returns, no second instance appears.
5. Start a second bench in Terminal (`pilot -b other start`). Pilot Desktop shows it as Attached. Stop the owned bench and confirm the attached one is unaffected. Restart the owned bench.
6. Open both dashboards. Confirm each shows its own session and signing in to one does not sign in the other. Use Reset dashboard session.
7. Quit with an owned bench running. The dialog lists it, Cancel keeps everything, Stop and quit stops only owned benches. Confirm the attached bench still runs.
8. Run a long task (a backup of a large site) and try Stop: it must refuse and explain.
9. Open in editor with Cursor installed, and with it renamed away (clear error).
10. Resize to the minimum 760 by 560 and check layout, Tab order and Escape in every dialog.
11. `lsof -iTCP -sTCP:LISTEN -nP | grep 7000` shows `127.0.0.1` for benches started by Desktop.
