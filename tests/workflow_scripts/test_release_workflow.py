#!/usr/bin/env python3
# file: tests/workflow_scripts/test_release_workflow.py
# version: 1.0.0
# guid: c3d4e5f6-a7b8-9c0d-1e2f-3a4b5c6d7e8f

"""Unit tests for release_workflow module."""

from __future__ import annotations

import sys
from pathlib import Path
from unittest.mock import MagicMock

import pytest

SCRIPTS_DIR = Path(__file__).resolve().parents[2] / ".github/workflows/scripts"
sys.path.insert(0, str(SCRIPTS_DIR))

import release_workflow  # pylint: disable=wrong-import-position
import workflow_common  # pylint: disable=wrong-import-position


@pytest.fixture(autouse=True)
def reset_config_cache() -> None:
    """Reset cached configuration between tests."""
    workflow_common._CONFIG_CACHE = None  # type: ignore[attr-defined]
    yield
    workflow_common._CONFIG_CACHE = None  # type: ignore[attr-defined]


def test_release_info_main_branch() -> None:
    """ReleaseInfo creates semantic tag for main branch."""
    info = release_workflow.ReleaseInfo(
        version="1.2.3",
        branch="main",
        language="go",
        language_version=None,
        is_stable_branch=False,
    )

    assert info.tag == "v1.2.3"


def test_release_info_stable_branch() -> None:
    """ReleaseInfo includes language version for stable branch."""
    info = release_workflow.ReleaseInfo(
        version="1.2.3",
        branch="stable-1-go-1.24",
        language="go",
        language_version="1.24",
        is_stable_branch=True,
    )

    assert info.tag == "v1.2.3-go124"


def test_detect_primary_language_priority(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """detect_primary_language honours priority order."""
    monkeypatch.chdir(tmp_path)
    (tmp_path / "go.mod").write_text(
        "module example.com/test",
        encoding="utf-8",
    )
    (tmp_path / "package.json").write_text(
        '{"name": "test"}',
        encoding="utf-8",
    )

    language = release_workflow.detect_primary_language()

    assert language == "go"


def test_detect_primary_language_rust(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """detect_primary_language identifies Rust repositories."""
    monkeypatch.chdir(tmp_path)
    (tmp_path / "Cargo.toml").write_text(
        '[package]\nname="test"',
        encoding="utf-8",
    )

    assert release_workflow.detect_primary_language() == "rust"


def test_detect_primary_language_missing(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """detect_primary_language raises when language cannot be determined."""
    monkeypatch.chdir(tmp_path)

    with pytest.raises(workflow_common.WorkflowError):
        release_workflow.detect_primary_language()


def test_get_branch_language_version_variants() -> None:
    """get_branch_language_version extracts version for stable branches."""
    assert release_workflow.get_branch_language_version(
        "stable-1-go-1.24",
        "go",
    ) == "1.24"
    assert release_workflow.get_branch_language_version(
        "stable-1-python-3.13",
        "python",
    ) == "3.13"
    assert release_workflow.get_branch_language_version("main", "go") is None
    assert release_workflow.get_branch_language_version(
        "stable-1-go-1.24",
        "python",
    ) is None


def test_is_stable_branch() -> None:
    """is_stable_branch returns True only for stable branches."""
    assert release_workflow.is_stable_branch("stable-1-go-1.24") is True
    assert release_workflow.is_stable_branch("main") is False


def test_get_current_branch_success(monkeypatch: pytest.MonkeyPatch) -> None:
    """get_current_branch returns branch name from git."""
    mock_result = MagicMock()
    mock_result.stdout = "main\n"
    monkeypatch.setattr(
        release_workflow.subprocess,
        "run",
        MagicMock(return_value=mock_result),
    )

    assert release_workflow.get_current_branch() == "main"


def test_get_current_branch_failure(monkeypatch: pytest.MonkeyPatch) -> None:
    """get_current_branch raises WorkflowError on git failure."""
    import subprocess

    monkeypatch.setattr(
        release_workflow.subprocess,
        "run",
        MagicMock(side_effect=subprocess.CalledProcessError(1, "git")),
    )

    with pytest.raises(workflow_common.WorkflowError):
        release_workflow.get_current_branch()


@pytest.mark.parametrize(
    ("language", "file_name", "content"),
    [
        ("go", "go.mod", "module example.com/test // v1.2.3"),
        ("rust", "Cargo.toml", '[package]\nversion = "1.2.3"'),
        ("python", "pyproject.toml", '[project]\nversion = "1.2.3"'),
        ("node", "package.json", '{"version": "1.2.3"}'),
    ],
)
def test_extract_version_from_file(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    language: str,
    file_name: str,
    content: str,
) -> None:
    """extract_version_from_file parses semantic versions per language."""
    monkeypatch.chdir(tmp_path)
    (tmp_path / file_name).write_text(content, encoding="utf-8")

    assert release_workflow.extract_version_from_file(language) == "1.2.3"


def test_extract_version_from_file_missing(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """extract_version_from_file raises error when manifest missing."""
    monkeypatch.chdir(tmp_path)

    with pytest.raises(workflow_common.WorkflowError):
        release_workflow.extract_version_from_file("go")


def test_create_release_info_main(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """create_release_info assembles release details for main branch."""
    monkeypatch.chdir(tmp_path)
    (tmp_path / "go.mod").write_text(
        "module example.com/test // v1.2.3",
        encoding="utf-8",
    )
    monkeypatch.setattr(release_workflow, "get_current_branch", lambda: "main")

    info = release_workflow.create_release_info()

    assert info.language == "go"
    assert info.version == "1.2.3"
    assert info.tag == "v1.2.3"
    assert info.is_stable_branch is False


def test_create_release_info_stable(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """create_release_info handles stable branch tagging."""
    monkeypatch.chdir(tmp_path)
    (tmp_path / "go.mod").write_text(
        "module example.com/test // v1.2.3",
        encoding="utf-8",
    )
    monkeypatch.setattr(
        release_workflow,
        "get_current_branch",
        lambda: "stable-1-go-1.24",
    )

    info = release_workflow.create_release_info()

    assert info.tag == "v1.2.3-go124"
    assert info.language_version == "1.24"
    assert info.is_stable_branch is True


def test_generate_changelog_categorization(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generate_changelog categorizes commits using conventional prefixes."""
    commits = [
        "feat: add new API endpoint",
        "fix: resolve bug",
        "chore: update deps",
    ]

    mock_tag = MagicMock()
    mock_tag.stdout = "v1.0.0\n"
    mock_log = MagicMock()
    mock_log.stdout = "\n".join(commits) + "\n"

    def fake_run(cmd, **_: object):  # type: ignore[override]
        if "describe" in cmd:
            return mock_tag
        return mock_log

    monkeypatch.setattr(release_workflow.subprocess, "run", fake_run)

    changelog = release_workflow.generate_changelog()

    assert "## ✨ Features" in changelog
    assert "feat: add new API endpoint" in changelog
    assert "## 🐛 Bug Fixes" in changelog
    assert "fix: resolve bug" in changelog
    assert "## 📝 Other Changes" in changelog
    assert "chore: update deps" in changelog


def test_generate_changelog_breaking_change(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generate_changelog highlights breaking changes."""
    mock_tag = MagicMock()
    mock_tag.stdout = "v1.0.0\n"
    mock_log = MagicMock()
    mock_log.stdout = "feat!: remove deprecated API\n"

    def fake_run(cmd, **_: object):  # type: ignore[override]
        if "describe" in cmd:
            return mock_tag
        return mock_log

    monkeypatch.setattr(release_workflow.subprocess, "run", fake_run)

    changelog = release_workflow.generate_changelog()

    assert "## ⚠️ Breaking Changes" in changelog
    assert "feat!: remove deprecated API" in changelog


def test_generate_changelog_no_commits(monkeypatch: pytest.MonkeyPatch) -> None:
    """generate_changelog returns placeholder when no commits found."""
    mock_tag = MagicMock()
    mock_tag.stdout = "v1.0.0\n"
    mock_log = MagicMock()
    mock_log.stdout = "\n"

    def fake_run(cmd, **_: object):  # type: ignore[override]
        if "describe" in cmd:
            return mock_tag
        return mock_log

    monkeypatch.setattr(release_workflow.subprocess, "run", fake_run)

    changelog = release_workflow.generate_changelog()

    assert "No notable changes" in changelog
