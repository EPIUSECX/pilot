from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import pytest

REPOSITORY = Path(__file__).resolve().parents[3]
BRIDGE = REPOSITORY / "desktop" / "bridge" / "pilot_desktop_bridge.py"


@pytest.fixture
def pilot_root(tmp_path: Path) -> Path:
    """A throwaway Pilot root (path with spaces) that borrows this checkout's `pilot` package."""
    root = tmp_path / "Pilot Root"
    (root / "bin").mkdir(parents=True)
    (root / "bin" / "pilot").write_text("#!/usr/bin/env python3\n")
    (root / "pilot").symlink_to(REPOSITORY / "pilot")
    return root


def make_bench(root: Path, name: str) -> Path:
    from pilot.config import BenchConfig

    directory = root / "benches" / name
    directory.mkdir(parents=True)
    BenchConfig.default(name, benches_root=root / "benches").write(directory)
    return directory


def run_bridge(root: Path, command: str) -> tuple[int, dict]:
    result = subprocess.run(
        [sys.executable, "-I", str(BRIDGE), "--pilot-root", str(root), command],
        capture_output=True,
        text=True,
        check=False,
    )
    return result.returncode, json.loads(result.stdout)


def test_inventory_is_empty_without_benches_directory(pilot_root):
    code, payload = run_bridge(pilot_root, "inventory")
    assert code == 0
    assert payload["schema_version"] == 1
    assert payload["benches"] == []


def test_inventory_reads_benches_with_spaces_in_path(pilot_root):
    make_bench(pilot_root, "alpha")
    make_bench(pilot_root, "beta")
    _, payload = run_bridge(pilot_root, "inventory")
    names = [bench["name"] for bench in payload["benches"]]
    assert names == ["alpha", "beta"]
    alpha = payload["benches"][0]
    assert " " in alpha["path"]
    assert alpha["read_error"] is None
    assert alpha["is_initialized"] is False
    assert alpha["mode"] == "development"
    assert isinstance(alpha["admin_port"], int)


def test_unreadable_bench_is_listed_with_its_error(pilot_root):
    broken = pilot_root / "benches" / "broken"
    broken.mkdir(parents=True)
    (broken / "bench.toml").write_text("not = [valid")
    _, payload = run_bridge(pilot_root, "inventory")
    assert payload["benches"][0]["name"] == "broken"
    assert payload["benches"][0]["read_error"]


def test_directory_without_bench_toml_is_ignored(pilot_root):
    (pilot_root / "benches" / "stray").mkdir(parents=True)
    _, payload = run_bridge(pilot_root, "inventory")
    assert payload["benches"] == []


def test_invalid_installation_reports_a_category(tmp_path):
    code, payload = run_bridge(tmp_path, "inventory")
    assert code == 1
    assert payload["error"]["category"] == "invalid_installation"


def test_identity_reports_versions(pilot_root):
    (pilot_root / "VERSION").write_text("v9.9.9\n")
    _, payload = run_bridge(pilot_root, "identity")
    assert payload["pilot_version"] == "v9.9.9"
    assert payload["python_version"].count(".") == 2


def run_bridge_with(root: Path, *arguments: str) -> tuple[int, dict]:
    result = subprocess.run(
        [sys.executable, "-I", str(BRIDGE), "--pilot-root", str(root), *arguments],
        capture_output=True,
        text=True,
        check=False,
    )
    return result.returncode, json.loads(result.stdout)


def test_activity_of_an_idle_bench(pilot_root):
    make_bench(pilot_root, "alpha")
    code, payload = run_bridge_with(pilot_root, "--bench", "alpha", "activity")
    assert code == 0
    assert payload["activity"]["running_tasks"] == 0
    assert set(payload["activity"]) >= {"active", "uncertain", "queued_tasks"}


@pytest.mark.parametrize("name", ["missing", "../alpha", ""])
def test_activity_rejects_unknown_or_escaping_bench_names(pilot_root, name):
    make_bench(pilot_root, "alpha")
    code, payload = run_bridge_with(pilot_root, "--bench", name, "activity")
    assert code == 1
    assert payload["error"]["category"] == "unknown_bench"
