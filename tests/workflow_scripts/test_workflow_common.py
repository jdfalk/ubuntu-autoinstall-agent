#!/usr/bin/env python3
# file: tests/workflow_scripts/test_workflow_common.py
# version: 1.0.0
# guid: b2c3d4e5-f6a7-8b9c-0d1e-2f3a4b5c6d7e

"""Unit tests for workflow_common module."""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Generator

import pytest

sys.path.insert(
    0,
    str(Path(__file__).resolve().parent.parent.parent / ".github/workflows/scripts"),
)

import workflow_common  # pylint: disable=wrong-import-position


@pytest.fixture(autouse=True)
def reset_config_cache() -> Generator[None, None, None]:
    """Reset global config cache between tests."""
    workflow_common._CONFIG_CACHE = None  # type: ignore[attr-defined]
    yield
    workflow_common._CONFIG_CACHE = None  # type: ignore[attr-defined]


def test_workflow_error_formatting() -> None:
    """WorkflowError includes message, hint, and doc link."""
    error = workflow_common.WorkflowError(
        "Test error",
        hint="Try this fix",
        docs_url="https://example.com/docs",
    )

    result = str(error)

    assert "❌ Test error" in result
    assert "💡 Hint: Try this fix" in result
    assert "📚 Docs: https://example.com/docs" in result


def test_append_to_file_missing_env(monkeypatch: pytest.MonkeyPatch) -> None:
    """append_to_file raises when env variable missing."""
    monkeypatch.delenv("GITHUB_OUTPUT", raising=False)

    with pytest.raises(workflow_common.WorkflowError) as exc_info:
        workflow_common.append_to_file("GITHUB_OUTPUT", "test")

    assert "GITHUB_OUTPUT" in str(exc_info.value)


def test_write_output_appends(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """write_output appends key-value pair to output file."""
    output_file = tmp_path / "output.txt"
    output_file.write_text("", encoding="utf-8")
    monkeypatch.setenv("GITHUB_OUTPUT", str(output_file))

    workflow_common.write_output("my_var", "my_value")

    content = output_file.read_text(encoding="utf-8")
    assert "my_var=my_value\n" in content


def test_get_repository_config_missing_file(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """get_repository_config raises when config not present."""
    monkeypatch.chdir(tmp_path)

    with pytest.raises(workflow_common.WorkflowError) as exc_info:
        workflow_common.get_repository_config()

    assert "repository-config.yml not found" in str(exc_info.value)


def test_get_repository_config_caches(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    """Config loaded once and cached for subsequent calls."""
    config_file = tmp_path / ".github" / "repository-config.yml"
    config_file.parent.mkdir(parents=True)
    config_file.write_text("test: value\n", encoding="utf-8")
    monkeypatch.chdir(tmp_path)

    first = workflow_common.get_repository_config()
    second = workflow_common.get_repository_config()

    assert first is second
    assert first["test"] == "value"


def test_config_path_nested(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """config_path retrieves nested value."""
    config_file = tmp_path / ".github" / "repository-config.yml"
    config_file.parent.mkdir(parents=True)
    config_file.write_text(
        "languages:\n  versions:\n    go: ['1.23', '1.24']\n",
        encoding="utf-8",
    )
    monkeypatch.chdir(tmp_path)

    result = workflow_common.config_path([], "languages", "versions", "go")

    assert result == ["1.23", "1.24"]


def test_config_path_default(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """config_path returns default when key missing."""
    config_file = tmp_path / ".github" / "repository-config.yml"
    config_file.parent.mkdir(parents=True)
    config_file.write_text("empty: {}\n", encoding="utf-8")
    monkeypatch.chdir(tmp_path)

    result = workflow_common.config_path(42, "missing", "key")

    assert result == 42


def test_timed_operation_records_summary(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    """timed_operation prints duration and writes summary."""
    summary_file = tmp_path / "summary.md"
    summary_file.write_text("", encoding="utf-8")
    monkeypatch.setenv("GITHUB_STEP_SUMMARY", str(summary_file))

    with workflow_common.timed_operation("Test operation"):
        pass

    output = capsys.readouterr().out
    assert "⏱️  Test operation took" in output
    summary_contents = summary_file.read_text(encoding="utf-8")
    assert "| Test operation |" in summary_contents


def test_sanitize_log_masks_tokens() -> None:
    """sanitize_log masks GitHub tokens and bearer values."""
    message = (
        "Token: ghp_abcdefghijklmnopqrstuvwxyz0123456789abcd "
        "Bearer ghb_fake_token"
    )

    result = workflow_common.sanitize_log(message)

    assert "***GITHUB_TOKEN***" in result
    assert "Bearer ***TOKEN***" in result


def test_ensure_file_creates(tmp_path: Path) -> None:
    """ensure_file creates file with content."""
    target = tmp_path / "subdir" / "file.txt"

    created = workflow_common.ensure_file(target, "content\n")

    assert created is True
    assert target.exists()
    assert target.read_text(encoding="utf-8") == "content\n"


def test_ensure_file_idempotent(tmp_path: Path) -> None:
    """ensure_file returns False when file already exists."""
    target = tmp_path / "file.txt"
    target.write_text("original\n", encoding="utf-8")

    created = workflow_common.ensure_file(target, "new\n")

    assert created is False
    assert target.read_text(encoding="utf-8") == "original\n"


def test_get_feature_flag_returns_default(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """get_feature_flag falls back to provided default."""
    config_file = tmp_path / ".github" / "repository-config.yml"
    config_file.parent.mkdir(parents=True)
    config_file.write_text("workflows: {}\n", encoding="utf-8")
    monkeypatch.chdir(tmp_path)

    result = workflow_common.get_feature_flag("use_new_ci", default=True)

    assert result is True


def test_get_and_require_feature_flag(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Feature flag helpers respect configuration values."""
    config_file = tmp_path / ".github" / "repository-config.yml"
    config_file.parent.mkdir(parents=True)
    config_file.write_text(
        "workflows:\n  experimental:\n    use_new_ci: true\n",
        encoding="utf-8",
    )
    monkeypatch.chdir(tmp_path)

    flag_enabled = workflow_common.get_feature_flag("use_new_ci")
    assert flag_enabled is True

    workflow_common.require_feature_flag("use_new_ci")

    config_file.write_text(
        "workflows:\n  experimental:\n    use_new_ci: false\n",
        encoding="utf-8",
    )
    workflow_common._CONFIG_CACHE = None  # type: ignore[attr-defined]

    with pytest.raises(workflow_common.WorkflowError) as exc_info:
        workflow_common.require_feature_flag("use_new_ci")

    assert "Feature 'use_new_ci' not enabled" in str(exc_info.value)
