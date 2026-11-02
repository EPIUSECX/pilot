"""Read-only JSON adapter over Pilot's bench inventory, versioned by SCHEMA_VERSION.

Run with the interpreter Pilot Desktop resolved (`python -I`), outside the Pilot install:

    python -I pilot_desktop_bridge.py --pilot-root ~/pilot inventory
    python -I pilot_desktop_bridge.py --pilot-root ~/pilot identity
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

SCHEMA_VERSION = 1
MINIMUM_PYTHON = (3, 11)
REQUIRED_INTERFACES = {
    "pilot.config": ("BenchConfig",),
    "pilot.core.bench": ("Bench",),
}


class BridgeError(Exception):
    def __init__(self, category: str, message: str) -> None:
        super().__init__(message)
        self.category = category
        self.message = message


def validate_pilot_root(value: str) -> Path:
    root = Path(value).expanduser().resolve()
    if not (root / "bin" / "pilot").is_file() or not (root / "pilot" / "__init__.py").is_file():
        raise BridgeError("invalid_installation", f"{root} is not a Pilot installation.")
    return root


def load_pilot(root: Path) -> None:
    if sys.version_info < MINIMUM_PYTHON:
        raise BridgeError("incompatible_python", "Pilot needs Python 3.11 or later.")
    sys.path.insert(0, str(root))
    import importlib

    for module_name, names in REQUIRED_INTERFACES.items():
        module = importlib.import_module(module_name)
        missing = [name for name in names if not hasattr(module, name)]
        if missing:
            raise BridgeError("incompatible_pilot", f"{module_name} lacks {', '.join(missing)}.")


def pilot_version(root: Path) -> str:
    version_file = root / "VERSION"
    return version_file.read_text(encoding="utf-8").strip() if version_file.is_file() else "dev"


def describe_bench(bench_directory: Path) -> dict:
    from pilot.config import BenchConfig
    from pilot.core.bench import Bench

    record = {
        "name": bench_directory.name,
        "path": str(bench_directory),
        "read_error": None,
    }
    try:
        config = BenchConfig.read(bench_directory, validate=False)
        admin = config.admin
        production = config.production.enabled
        record.update(
            name=config.name or bench_directory.name,
            mode="production" if production else "development",
            admin_enabled=admin.enabled,
            admin_port=admin.port,
            admin_domain=admin.domain,
            is_initialized=Bench(config, bench_directory).is_initialized,
            database_type=config.db_type,
            frappe_branch=_framework_branch(config),
        )
    except Exception as error:
        record["read_error"] = f"{type(error).__name__}: {error}"
    return record


def _framework_branch(config) -> str | None:
    try:
        return config.framework_app.branch or None
    except Exception:
        return None


def list_benches(root: Path) -> list[dict]:
    benches_directory = root / "benches"
    if not benches_directory.is_dir():
        return []
    candidates = sorted(benches_directory.iterdir())
    return [describe_bench(path) for path in candidates if (path / "bench.toml").is_file()]


def read_activity(root: Path, bench_name: str) -> dict:
    """Task activity from Pilot's on-disk task store; `uncertain` means safety is unproven."""
    from pilot.managers.task.activity import TaskActivityReader

    bench_directory = (root / "benches" / bench_name).resolve()
    if (
        bench_directory.parent != (root / "benches").resolve()
        or not (bench_directory / "bench.toml").is_file()
    ):
        raise BridgeError("unknown_bench", f"No bench named {bench_name!r}.")
    activity = TaskActivityReader(bench_directory).read()
    return {
        "active": activity.active,
        "uncertain": activity.uncertain,
        "queued_tasks": activity.queued_tasks,
        "running_tasks": activity.running_tasks,
        "worker_status": activity.worker_status,
        "current_task_id": activity.current_task_id,
    }


def run(arguments: argparse.Namespace) -> dict:
    root = validate_pilot_root(arguments.pilot_root)
    load_pilot(root)
    payload = {"schema_version": SCHEMA_VERSION, "pilot_root": str(root)}
    if arguments.command == "identity":
        payload.update(pilot_version=pilot_version(root), python_version=sys.version.split()[0])
    elif arguments.command == "activity":
        payload["activity"] = read_activity(root, arguments.bench)
    else:
        payload["benches"] = list_benches(root)
    return payload


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--pilot-root", required=True)
    parser.add_argument("--bench", default="", help="Bench name for `activity`.")
    parser.add_argument("command", choices=["inventory", "identity", "activity"])
    return parser


def main(argv: list[str] | None = None) -> int:
    arguments = build_parser().parse_args(argv)
    try:
        payload = run(arguments)
        code = 0
    except BridgeError as error:
        payload = {
            "schema_version": SCHEMA_VERSION,
            "error": {"category": error.category, "message": error.message},
        }
        code = 1
    json.dump(payload, sys.stdout)
    sys.stdout.write("\n")
    return code


if __name__ == "__main__":
    raise SystemExit(main())
