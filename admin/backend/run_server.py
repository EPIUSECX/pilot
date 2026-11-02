from __future__ import annotations

import argparse
import ipaddress
import logging
import os
import subprocess
import threading
from collections.abc import Mapping
from pathlib import Path

BIND_HOST_ENV = "PILOT_ADMIN_HOST"
# "::" makes Werkzeug bind a dual-stack socket, so the admin is reachable
# over both IPv4 and IPv6 in dev (where there is no nginx in front).
DEFAULT_BIND_HOST = "::"


def resolve_bind_host(environ: Mapping[str, str] | None = None) -> str:
    """Address the admin listens on: `PILOT_ADMIN_HOST` when set, else every interface."""
    value = (os.environ if environ is None else environ).get(BIND_HOST_ENV, "").strip()
    if not value:
        return DEFAULT_BIND_HOST
    try:
        ipaddress.ip_address(value)
    except ValueError as error:
        raise SystemExit(f"{BIND_HOST_ENV} must be an IP address, got {value!r}.") from error
    return value


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Run the admin app with its own Werkzeug server (dev reload or wizard "
        "bootstrap) instead of behind gunicorn."
    )
    parser.add_argument("--bench-root", required=True)
    parser.add_argument("--port", type=int, default=8002)
    parser.add_argument("--timeout", type=int, default=900, help="Inactivity timeout in seconds")
    parser.add_argument(
        "--no-timeout",
        action="store_true",
        help="Disable inactivity watchdog (used when managed by procfile)",
    )
    parser.add_argument(
        "--dev", action="store_true", help="Enable auto-reload on code changes (development only)"
    )
    parser.add_argument("--wizard", action="store_true", help="Running as the standalone setup-wizard server")
    args = parser.parse_args()

    from admin.backend.app import create_app
    from pilot.config import BenchConfig

    bench_root = Path(args.bench_root)
    app = create_app(bench_root)
    app.config["WIZARD_SERVER"] = args.wizard

    from admin.backend.central_bootstrap import install_central_bootstrap_watcher

    install_central_bootstrap_watcher(app, bench_root)

    skip_watchdog = args.no_timeout or args.dev
    if not skip_watchdog:
        try:
            config = BenchConfig.read(bench_root)
            skip_watchdog = config.admin.enabled
        except Exception as exc:
            logging.debug("Could not read bench.toml to decide the watchdog: %s", exc)

    if not skip_watchdog:
        from admin.backend.watchdog import AdminProcessOwner, install_idle_watchdog

        install_idle_watchdog(
            app,
            bench_root,
            args.timeout,
            AdminProcessOwner.current(),
        )

    # Start vite only in the outer watcher process, not in the child that
    # werkzeug's reloader spawns (WERKZEUG_RUN_MAIN is set in that child).
    if args.dev and not os.environ.get("WERKZEUG_RUN_MAIN"):
        _start_vite_watch()

    if not args.dev or os.environ.get("WERKZEUG_RUN_MAIN"):
        from pilot.managers.task import TaskWorkerControl

        TaskWorkerControl(bench_root).start_background_worker()

    app.run(host=resolve_bind_host(), port=args.port, threaded=True, use_reloader=args.dev)


def _start_vite_watch() -> None:
    frontend_dir = Path(__file__).parent.parent / "frontend"
    if not frontend_dir.exists():
        return

    def _run() -> None:
        subprocess.run(
            ["node_modules/.bin/vite", "build", "--watch", "--mode", "development"],
            cwd=str(frontend_dir),
        )

    threading.Thread(target=_run, daemon=True).start()


if __name__ == "__main__":
    main()
