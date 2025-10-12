import argparse
import subprocess
from pathlib import Path
from typing import Any

import pytest

import ci_workflow


def test_debug_filter_outputs(capsys, monkeypatch):
    env_values = {
        "CI_GO_FILES": "true",
        "CI_FRONTEND_FILES": "false",
        "CI_PYTHON_FILES": "true",
        "CI_RUST_FILES": "false",
        "CI_DOCKER_FILES": "false",
        "CI_DOCS_FILES": "true",
        "CI_WORKFLOW_FILES": "true",
        "CI_LINT_FILES": "false",
    }
    for key, value in env_values.items():
        monkeypatch.setenv(key, value)

    ci_workflow.debug_filter(argparse.Namespace())
    output = capsys.readouterr().out
    for label in [
        "Go files changed: true",
        "Frontend files changed: false",
        "Python files changed: true",
        "Docs files changed: true",
        "Workflow files changed: true",
    ]:
        assert label in output


def test_determine_execution_sets_outputs(tmp_path, monkeypatch):
    output_file = tmp_path / "output.txt"
    monkeypatch.setenv("GITHUB_OUTPUT", str(output_file))
    monkeypatch.setenv("GITHUB_HEAD_COMMIT_MESSAGE", "fix bug [skip ci]")
    monkeypatch.setenv("CI_GO_FILES", "true")
    monkeypatch.setenv("CI_FRONTEND_FILES", "false")
    monkeypatch.setenv("CI_PYTHON_FILES", "true")
    monkeypatch.setenv("CI_RUST_FILES", "false")
    monkeypatch.setenv("CI_DOCKER_FILES", "true")

    ci_workflow.determine_execution(argparse.Namespace())
    lines = output_file.read_text().splitlines()
    assert "skip_ci=true" in lines
    assert "should_lint=true" in lines
    assert "should_test_go=true" in lines
    assert "should_test_frontend=false" in lines
    assert "should_test_python=true" in lines
    assert "should_test_docker=true" in lines


class DummyResponse:
    def __init__(self, status_code: int, payload: dict[str, Any]):
        self.status_code = status_code
        self._payload = payload

    def json(self) -> dict[str, Any]:
        return self._payload


def test_wait_for_pr_automation_completed(monkeypatch, capsys):
    monkeypatch.setenv("GITHUB_REPOSITORY", "owner/repo")
    monkeypatch.setenv("GITHUB_TOKEN", "token")
    monkeypatch.setenv("TARGET_SHA", "abc123")
    monkeypatch.setenv("WORKFLOW_NAME", "PR Automation")
    monkeypatch.setenv("MAX_ATTEMPTS", "1")

    def fake_get(url: str, **kwargs):
        assert "owner/repo" in url
        return DummyResponse(
            200,
            {
                "workflow_runs": [
                    {"head_sha": "abc123", "name": "PR Automation", "status": "completed"}
                ]
            },
        )

    monkeypatch.setattr(ci_workflow.requests, "get", fake_get)
    ci_workflow.wait_for_pr_automation(argparse.Namespace())
    captured = capsys.readouterr().out
    assert "✅ PR automation has completed" in captured


def test_load_super_linter_config(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    (tmp_path / "super-linter-ci.env").write_text("FOO=bar\n", encoding="utf-8")
    env_file = tmp_path / "env.txt"
    output_file = tmp_path / "output.txt"

    monkeypatch.setenv("EVENT_NAME", "push")
    monkeypatch.setenv("CI_ENV_FILE", "super-linter-ci.env")
    monkeypatch.setenv("PR_ENV_FILE", "super-linter-pr.env")
    monkeypatch.setenv("GITHUB_ENV", str(env_file))
    monkeypatch.setenv("GITHUB_OUTPUT", str(output_file))

    ci_workflow.load_super_linter_config(argparse.Namespace())
    assert env_file.read_text() == "FOO=bar\n"
    assert "config-file=super-linter-ci.env" in output_file.read_text()


def test_check_go_coverage_success(monkeypatch, tmp_path):
    coverage_file = tmp_path / "coverage.out"
    coverage_file.write_text("mode: set\n", encoding="utf-8")
    html_file = tmp_path / "coverage.html"

    monkeypatch.setenv("COVERAGE_FILE", str(coverage_file))
    monkeypatch.setenv("COVERAGE_HTML", str(html_file))
    monkeypatch.setenv("COVERAGE_THRESHOLD", "70")

    calls = []

    def fake_run(cmd, check=True, capture_output=False, text=False, **kwargs):
        calls.append((tuple(cmd), capture_output))
        if "-func" in cmd:
            return subprocess.CompletedProcess(cmd, 0, stdout="total: (statements) 75.0%\n")
        return subprocess.CompletedProcess(cmd, 0)

    monkeypatch.setattr(ci_workflow.shutil, "which", lambda name: "/usr/bin/go")
    monkeypatch.setattr(ci_workflow, "subprocess", subprocess)
    monkeypatch.setattr(ci_workflow.subprocess, "run", fake_run)

    ci_workflow.check_go_coverage(argparse.Namespace())
    assert any("-func" in part for command, _ in calls for part in command)


def test_generate_ci_summary_writes_summary(tmp_path, monkeypatch):
    summary_path = tmp_path / "summary.md"
    monkeypatch.setenv("GITHUB_STEP_SUMMARY", str(summary_path))
    monkeypatch.setenv("PRIMARY_LANGUAGE", "python")
    monkeypatch.setenv("HAS_RUST", "false")
    monkeypatch.setenv("HAS_GO", "true")
    monkeypatch.setenv("HAS_PYTHON", "true")
    monkeypatch.setenv("HAS_FRONTEND", "false")
    monkeypatch.setenv("HAS_DOCKER", "false")
    monkeypatch.setenv("JOB_DETECT_CHANGES", "success")
    monkeypatch.setenv("JOB_DETECT_LANGUAGES", "success")
    monkeypatch.setenv("JOB_CHECK_OVERRIDES", "success")
    monkeypatch.setenv("JOB_LINT", "success")
    monkeypatch.setenv("JOB_TEST_GO", "success")
    monkeypatch.setenv("JOB_TEST_FRONTEND", "skipped")
    monkeypatch.setenv("JOB_TEST_PYTHON", "success")
    monkeypatch.setenv("JOB_TEST_RUST", "skipped")
    monkeypatch.setenv("JOB_RUST_COVERAGE", "skipped")
    monkeypatch.setenv("JOB_TEST_DOCKER", "skipped")
    monkeypatch.setenv("JOB_TEST_DOCS", "success")
    monkeypatch.setenv("JOB_RELEASE_BUILD", "success")
    monkeypatch.setenv("JOB_SECURITY_SCAN", "success")
    monkeypatch.setenv("JOB_PERFORMANCE_TEST", "skipped")
    monkeypatch.setenv("CI_GO_FILES", "true")
    monkeypatch.setenv("CI_FRONTEND_FILES", "false")
    monkeypatch.setenv("CI_PYTHON_FILES", "true")
    monkeypatch.setenv("CI_RUST_FILES", "false")
    monkeypatch.setenv("CI_DOCKER_FILES", "false")
    monkeypatch.setenv("CI_DOCS_FILES", "true")
    monkeypatch.setenv("CI_WORKFLOW_FILES", "true")

    ci_workflow.generate_ci_summary(argparse.Namespace())
    content = summary_path.read_text()
    assert "# 🚀 CI Pipeline Summary" in content
    assert "| Lint | success |" in content
    assert "- Python: true" in content


def test_check_ci_status_failure(monkeypatch):
    monkeypatch.setenv("JOB_LINT", "failure")
    monkeypatch.setenv("JOB_TEST_GO", "success")
    monkeypatch.setenv("JOB_TEST_FRONTEND", "success")
    monkeypatch.setenv("JOB_TEST_PYTHON", "success")
    monkeypatch.setenv("JOB_TEST_RUST", "success")
    monkeypatch.setenv("JOB_TEST_DOCKER", "success")
    monkeypatch.setenv("JOB_RELEASE_BUILD", "success")

    with pytest.raises(SystemExit):
        ci_workflow.check_ci_status(argparse.Namespace())
