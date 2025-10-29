#!/usr/bin/env python3
# file: tests/integration/test_ci_workflow_integration.py
# version: 1.0.0
# guid: d4e5f6a7-b8c9-0d1e-2f3a-4b5c6d7e8f9a

"""Integration tests for CI workflow system."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(
    0,
    str(Path(__file__).resolve().parent.parent.parent / ".github/workflows/scripts"),
)

import ci_workflow  # pylint: disable=wrong-import-position


@pytest.mark.integration
def test_ci_workflow_end_to_end(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """
    Validate CI workflow from change detection to matrix generation.

    Ensures the system:
        1. Detects file changes in a git repository
        2. Generates an optimized test matrix
        3. Produces JSON suitable for GitHub Actions consumption
    """
    repo_dir = tmp_path / "test-repo"
    repo_dir.mkdir()
    monkeypatch.chdir(repo_dir)

    subprocess.run(["git", "init"], check=True)
    subprocess.run(["git", "config", "user.email", "test@example.com"], check=True)
    subprocess.run(["git", "config", "user.name", "Test User"], check=True)

    (repo_dir / "README.md").write_text("# Test\n", encoding="utf-8")
    subprocess.run(["git", "add", "."], check=True)
    subprocess.run(["git", "commit", "-m", "Initial commit"], check=True)

    config_dir = repo_dir / ".github"
    config_dir.mkdir()
    config_file = config_dir / "repository-config.yml"
    config_file.write_text(
        (
            "languages:\n"
            "  versions:\n"
            "    go: ['1.23', '1.24']\n"
            "    python: ['3.13', '3.14']\n"
            "build:\n"
            "  platforms:\n"
            "    - ubuntu-latest\n"
            "    - macos-latest\n"
        ),
        encoding="utf-8",
    )

    (repo_dir / "main.go").write_text("package main\n", encoding="utf-8")
    subprocess.run(["git", "add", "."], check=True)
    subprocess.run(["git", "commit", "-m", "Add Go code"], check=True)

    changes = ci_workflow.detect_changes(base_ref="HEAD~1")

    languages_to_test: list[str] = []
    for language in ["go", "python"]:
        if ci_workflow.should_run_tests(language, changes):
            languages_to_test.append(language)

    matrix = ci_workflow.generate_test_matrix(
        languages_to_test,
        optimize=True,
        branch_name="main",
    )

    assert changes.go_changed is True
    assert changes.python_changed is False
    assert "go" in languages_to_test
    assert "python" not in languages_to_test

    entries = matrix["include"]
    assert entries
    assert all(entry["language"] == "go" for entry in entries)

    matrix_json = json.dumps(matrix)
    parsed = json.loads(matrix_json)
    assert parsed == matrix


@pytest.mark.integration
def test_workflow_with_no_changes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Ensure CI workflow skips tests when only docs change."""
    repo_dir = tmp_path / "test-repo"
    repo_dir.mkdir()
    monkeypatch.chdir(repo_dir)

    subprocess.run(["git", "init"], check=True)
    subprocess.run(["git", "config", "user.email", "test@example.com"], check=True)
    subprocess.run(["git", "config", "user.name", "Test User"], check=True)

    (repo_dir / "README.md").write_text("# Test\n", encoding="utf-8")
    subprocess.run(["git", "add", "."], check=True)
    subprocess.run(["git", "commit", "-m", "Initial commit"], check=True)

    (repo_dir / "README.md").write_text("# Updated Test\n", encoding="utf-8")
    subprocess.run(["git", "add", "."], check=True)
    subprocess.run(["git", "commit", "-m", "Update docs"], check=True)

    changes = ci_workflow.detect_changes(base_ref="HEAD~1")

    languages_to_test: list[str] = []
    for language in ["go", "python"]:
        if ci_workflow.should_run_tests(language, changes):
            languages_to_test.append(language)

    assert changes.docs_changed is True
    assert changes.go_changed is False
    assert changes.python_changed is False
    assert not languages_to_test
