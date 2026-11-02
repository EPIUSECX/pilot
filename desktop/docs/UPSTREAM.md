# Upstream contracts

Everything below was read from the Pilot source at the pinned commit or from command help on 1 November 2026. Nothing is inferred from memory.

## Selected versions

| Item | Choice | Why |
| --- | --- | --- |
| Pilot | `develop` at `ee57c4b3c228792b5ab3c116684971c1e58ab4ee` (checkout of `EPIUSECX/pilot`, includes `frappe/pilot` PR 743) | No release tag exists in this checkout. The commit is pinned in `desktop/bridge/PILOT_REVISION` and the bridge refuses a Pilot whose `pilot/` tree lacks the interfaces it imports. |
| Installed Pilot seen on the dev Mac | `~/pilot`, `VERSION` file `v0.0.16-pre-alpha`, no `bin/pilot` | Shows that discovery must verify `bin/pilot` and `pilot/__init__.py` before calling an install usable. |
| `frappe-ui` | `1.1.0` (npm `latest` on 2026-11-01) | Stable 1.x. Peers: `vue >=3.5.0`, `tailwindcss >=3.4.2 <4`, `vite >=5` (optional), Node `>=20.19.0`. |
| Tailwind CSS | `3.4.x` | Required by the `frappe-ui` peer range. Tailwind 4 is excluded by `scripts/check-dependencies.mjs`. |
| Vue / Vite / TypeScript | `3.5.x` / `8.x` / `6.0.x` | Matches Pilot's own dashboard (`vite ^8.1.5`, `typescript ^6.0.3`). `vue-tsc` needs `typescript >=5`. |
| Tauri | 2.x (`@tauri-apps/cli` 2.12.4, `@tauri-apps/api` 2.12.4) | Tauri 2 is required by the brief. |
| Pilot dashboard `frappe-ui` | `^1.0.0-rc.1` | Unchanged. The dashboard is not migrated by this work. |
| Licenses | Pilot: AGPL-3.0. `frappe-ui`: MIT. Tauri: MIT or Apache-2.0. | `desktop/` lives in the Pilot repository, so it is distributed under the repository's AGPL-3.0 license. |

## 1. CLI entry point and discovery

- Installer: `install.sh` sets `PILOT_DIR="$HOME/pilot"`. A release install unpacks a tarball there. `--dev` clones `develop`.
- Entry point: `<root>/bin/pilot` is a `#!/usr/bin/env python3` script that puts `<root>` on `sys.path` and calls `pilot.cli.main`. It needs only the standard library and Python 3.11 or later.
- The shebang picks whichever `python3` is first on `PATH`. A Finder-launched app has a minimal `PATH`, where `/usr/bin/python3` is Python 3.9.6 on this Mac. Pilot Desktop therefore never runs `bin/pilot` through its shebang. It runs `[<python>=3.11, <root>/bin/pilot, ...]` with an interpreter it resolved and version-checked.
- `pilot.utils.cli_root()` is the parent of the `pilot` package. The benches directory is `<root>/benches` (`pilot.utils.benches_dir()`).
- `pilot --version` prints `dev` for a source checkout. `VERSION` exists only in release tarballs.

## 2. Bench inventory

- Source of truth: `<root>/benches/<name>/bench.toml`, read through `pilot.config.BenchConfig.read(path, validate=False)`.
- `pilot ls` prints a decorated table with ANSI colour and no JSON mode. Desktop never parses it.
- Adapter: `desktop/bridge/pilot_desktop_bridge.py` imports only `BenchConfig`, `Bench.is_initialized` and `pilot.utils.admin_url`, and prints versioned JSON (`schema_version: 1`). It is run with `python -I` from the bundled resources, outside the Pilot install.
- Fields used: `name`, `production.enabled`, `admin.enabled`, `admin.port`, `admin.domain`, `db_type`, the framework app branch, and whether the bench is initialized.

## 3. Create, start, stop

- Create: `pilot new NAME [--frappe-branch BRANCH] [--database mariadb|postgres|sqlite] [--admin-password PW]`. Default branch is `version-16`. Pilot writes `bench.toml`, picks a per-bench port offset, enables the Admin (`admin_enabled: True`), and refuses an existing bench with `BenchAlreadyExistsError`.
- Without `--admin-password` and without a TTY, Pilot generates a password and prints `Generated admin password: <value>`. Desktop never passes a password on argv. It redacts the line from every log and returns the value to the launcher once, in memory, for one reveal.
- Start: `pilot -b NAME start`. A development bench runs in the foreground, its children get no stdin, and each child is started with `setsid`, so children are not in the supervisor's process group.
- Before starting, `Bench.start` calls a best-effort `ProcessManager.stop()` on the same bench. Starting a bench that is already running would stop it. Desktop must attach first and never call `start` on a live bench.
- Uninitialized bench: `pilot start` runs `admin.backend.run_server --wizard` through `subprocess.run`, with no pid file, and prints `http://localhost:<port>/?sid=<token>`. The token is a one-hour setup link.
- Stop: `pilot -b NAME stop` reads `pids/bench.pid` and sends `SIGTERM` to that pid. The supervisor then drains its children (5 second grace, then `SIGKILL` per child group, datastores last). With no pid file it falls back to `ss`, which does not exist on macOS, so it reports `Bench is not running` for a wizard server.
- Restart: `pilot restart` is production only and raises on a development bench. Desktop restart is the verified stop then start sequence.
- `pilot drop`, `get-app`, `new-site` and every other workflow stay in the dashboard.

## 4. Admin entry URL, host and port

- Development URL: `http://localhost:<admin.port>` (`pilot.utils.admin_url`). `admin.port` defaults to 7000 plus the bench's port offset.
- `admin.backend.run_server` binds `host="::"`, which listens on every interface. There is no setting for the bind address.
- Integration fix in this repository: `run_server` reads `PILOT_ADMIN_HOST` (default `::`, so existing behaviour is unchanged). Desktop sets it to `127.0.0.1` for every `pilot start` it launches. A bench that was already running keeps whatever address it bound, and the bench summary reports that as `attached` without claiming it is private. See `docs/configuration.md`.
- The dashboard URL Desktop opens is `http://127.0.0.1:<port>`, not `localhost`, so an IPv6-first resolution cannot reach a different listener.

## 5. Readiness and identity

- Both routes live under `/api/v1`. The same paths at the root (`/health`, `/bootstrap`) return the dashboard page with status 200, so Desktop reads only `/api/v1/...` and requires JSON. Found on a real bench on 2 November 2026; the first version of Desktop probed the root paths and could never see a real bench as ready.
- `GET /api/v1/health` returns `{"status":"ok"}` with no authentication and `Access-Control-Allow-Origin: *`. It proves a Flask app answers, not which bench.
- `GET /api/v1/bootstrap` is unauthenticated and returns `mode` (`setup`, `pending`, `admin`), `enabled` and `name`, where `name` is `BenchConfig.name`.
- `ProcessManager.is_admin_running()` is only a TCP connect to `admin.port`, which any process satisfies.
- Desktop verification, all required: (1) `bench.toml` gives the expected name and port. (2) The process listening on that port, found with `lsof`, descends from the bench's live supervisor pid (`pids/bench.pid`), or from the process Desktop spawned, or, for a wizard started elsewhere, has a command line with `admin.backend.run_server` and this bench's `--bench-root`. (3) `/health` answers. (4) `/bootstrap` returns the expected `name`. A listener that fails (2) or (4) is a conflict, never a success.

## 6. Sign-in, session, logout

- Sessions are an `sid` cookie: `HttpOnly`, `SameSite=Lax`, `Secure` only when the bench is TLS, 24 hour lifetime for a password login.
- `?sid=<token>` on the root URL is exchanged for a cookie by `POST /auth/session`. Setup-link tokens last one hour and mint a 3 hour session. `Authorization: Bearer <token>` is also accepted.
- Logout deletes the cookie through `DELETE /auth/session` (the three `/auth/session` methods are the only authenticated-free session routes besides `/health` and `/bootstrap`).
- Cookies are keyed by host, not port. Two benches on `127.0.0.1` would share an `sid` cookie name. Desktop isolates them with a separate WebKit data store per bench (see ARCHITECTURE.md) and falls back to one active dashboard plus a session reset.

## 7. APIs and task events

- Flask JSON API under `admin/backend/api/v1`. Not Frappe DocType endpoints. Desktop calls only `/api/v1/health` and `/api/v1/bootstrap`; the dashboard makes every other call itself.
- Task events are Server-Sent Events: `GET /tasks/<task_id>/events` and a second stream in `tasks.py` (`text/event-stream`). Desktop does not subscribe. The dashboard consumes them inside its own window.
- Long work returns `{"task_id", "created"}`. Active-task state is also on disk: `pilot.managers.task.activity.TaskActivityReader(bench_root).read()` returns `active`, `uncertain`, `queued_tasks` and `running_tasks` from `<bench>/tasks` with no login. The bridge's `activity` command wraps it, and an active or uncertain result blocks Stop and Restart.

## 8. Process tree and shared services

- Dev `pilot start` writes `pids/bench.pid` (its own pid) and `pids/<process>.pid` per child. It starts `redis_cache`, `redis_queue`, web, socketio, workers, a watcher, and the Admin when `admin.enabled`.
- Children are `setsid` leaders. Desktop identifies owned processes by walking descendants of the process it spawned, not by process name.
- MariaDB and PostgreSQL are shared, per-user services managed by Pilot's database managers (`pilot init` enables the unit at boot). Desktop never starts, stops or restarts them.

## 9. Runtime requirements

- Host Python for the CLI: 3.11 or later, standard library only. Benches get their own Python 3.14 through `uv` (a separate environment, never conflated with the host interpreter).
- Node.js 24 (README and `install.sh` check `>= 24`). Frappe v16 or later.
- macOS packages from `install.sh`: `mariadb@11.8`, `postgresql@16`, `redis`, `nginx`, `certbot`, plus `git`, `brew`, `node`, `python3`.
- Desktop checks: Python at least 3.11, Node at least 24, `git`, `uv`, Homebrew, `mariadb`/`postgresql`/`redis` executables, and whether the services answer. Missing, incompatible and stopped are separate states.

## 10. Frontend dependencies and notices

- Pilot dashboard: `frappe-ui ^1.0.0-rc.1`, `vue ^3.5.0`, `tailwindcss ^3.4.0`, `vite ^8.1.5`, `typescript ^6.0.3`, `vue-tsc ^3.3.7`.
- Desktop shell: independent `package.json`, pinned exactly, committed `package-lock.json`.
- No Pilot dashboard source is copied into the shell.

## Decisions

1. The repository is Pilot, so desktop code is in `desktop/`.
2. The launcher is served from bundled assets. The dashboard opens in a separate window on its real `http://127.0.0.1:<port>` origin, with no native command access.
3. The Python adapter exists because stopped benches have no JSON interface.
4. One upstream change: `PILOT_ADMIN_HOST` in `admin/backend/run_server.py`, with a test and a configuration note.
