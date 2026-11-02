# Architecture

## Boundaries

| Component | Responsibility |
| --- | --- |
| Vue launcher (`src/`) | Frappe UI v1 screens, user actions, local runtime status. Served from bundled assets. |
| Tauri native layer (`src-tauri/`) | Executable discovery, validated commands, process supervision, preferences, tray, dashboard windows. |
| Pilot runtime and dashboard | Bench initialization, sites, apps, backups, tasks. Reached through the supported CLI and the Admin dashboard. |
| Python bridge (`bridge/`) | Versioned JSON for the bench inventory and task activity, because stopped benches have no JSON interface. |

The launcher never calls Pilot's Admin API. The dashboard is Pilot's own page in a separate window on its real `http://127.0.0.1:<port>` origin, so its normal authentication and API behavior are unchanged.

## Native permissions

- Every app command is listed in `src-tauri/build.rs`, which makes Tauri generate one `allow-<command>` permission each. `capabilities/launcher.json` grants exactly those, plus event listen and unlisten, to the window labelled `main`. There is no other capability file, no `remote` section and no permission for dashboard windows.
- Each command also calls `authorize_webview`, which requires the label `main` and the bundled origin (`tauri://localhost`, or the Vite dev server in debug builds). A launcher that navigated to a web page or a Pilot page is refused.
- The renderer supplies stable bench ids and form values, never executables or paths. The Pilot folder comes from a native folder chooser and is validated in Rust. Editors and Finder open a path taken from Pilot's own inventory.
- The webview keeps a content security policy. No CORS, TLS or webview security setting is relaxed.
- Tests: `tests/ipc_authorization.rs` runs Tauri's mock runtime against the real generated ACL, and `tests/capabilities.rs` checks that the command list, manifest, capability and CSP agree.

## Process ownership

- Starting runs `python <root>/bin/pilot -b <bench> start` with an explicit argument array, the Pilot folder as working directory, no stdin, its own process group and a controlled environment (`HOME`, verified `PATH`, `LANG`, `PILOT_ADMIN_HOST=127.0.0.1`). The interpreter is a resolved Python 3.11 or later, never the shebang.
- Ownership is recorded immediately: pid, start time and the argument string Desktop passed (not the executable path, which an interpreter shim can replace by exec), held in memory and written to `owned-processes.json` (mode 0600). A match needs all three, so a reused pid never qualifies.
- Before any start, Pilot Desktop probes the bench. A running bench is attached, not launched, because `pilot start` first stops a running copy of the same bench.
- Identity is verified in four parts: the listener on the admin port descends from the live `pids/bench.pid` supervisor (or from the process Desktop spawned, or is a setup wizard whose command line names this bench), `/health` answers, and `/bootstrap` reports the expected bench name. A listener that fails any part is a conflict.
- Stop of an owned bench sends `SIGTERM` to the supervisor and waits for Pilot's own drain. Only if the grace period passes does it `SIGKILL` the verified supervisor and its descendants, and it reports how many it stopped. It never stops MariaDB, PostgreSQL or Redis services, and never matches by process name.
- Stop or Restart of an attached bench needs explicit confirmation of the effect, then uses `pilot stop`. A bench that does not stop is reported, never force-killed.
- Before any stop or restart, the bridge reads Pilot's on-disk task state. Active or unknown work blocks the operation with an explanation, and the app stays open.
- Lifecycle operations for one bench are serialized. A second request fails with a busy error.
- On launch, saved records are reconciled with the live process table and discarded. A survivor is shown as attached and is not claimed or killed.
- A periodic probe every 10 seconds corrects stale status, which also covers wake from sleep. Nothing restarts automatically.

## Sessions

- Each bench dashboard window uses its own WebKit data store, derived from the bench path (`data_store_identifier`, macOS 14 or later). Cookies for bench A never reach bench B, even though both are on `127.0.0.1`.
- Reset dashboard session clears only that window's store.
- Navigation and new-window requests stay in the window only for the exact dashboard origin. Other links open in the system browser. Downloads are accepted only from the dashboard origin and saved under `~/Downloads` with a sanitized name.
- The setup link printed by `pilot start` (`?sid=<token>`) is read from process output in memory, accepted only for `localhost` or `127.0.0.1` on this bench's admin port, rewritten to `127.0.0.1`, used once to open the dashboard, and never logged. No token is copied into an external browser URL.
- No credential is stored. The generated Admin password printed by `pilot new` is returned once to the launcher for display and is redacted from every log.

## Data

Preferences (`preferences.json`, schema version 1, atomic writes, mode 0600), sanitized logs (1 MB per bench with three rotated files) and the ownership record live under the macOS application support and log directories, outside the app bundle. Removing the app deletes no Pilot data.
