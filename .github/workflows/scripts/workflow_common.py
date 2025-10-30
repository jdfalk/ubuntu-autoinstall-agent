#!/usr/bin/env python3
# file: .github/workflows/scripts/workflow_common.py
# version: 1.0.0
# guid: e5f6a7b8-c9d0-1e2f-3a4b-5c6d7e8f9a0b

"""Shared utilities for GitHub Actions workflow helper scripts."""

from __future__ import annotations

from contextlib import contextmanager
import os
from pathlib import Path
import sys
import time
from typing import Any

import yaml

_CONFIG_CACHE: dict[str, Any] | None = None


class WorkflowError(Exception):
    """Workflow execution error with optional hints and documentation links."""

    def __init__(
        self,
        message: str,
        hint: str = "",
        docs_url: str = "",
    ) -> None:
        """Initialize workflow error."""
        super().__init__(message)
        self.message = message
        self.hint = hint
        self.docs_url = docs_url

    def __str__(self) -> str:
        """Format error with hints and documentation links."""
        parts = [f"❌ {self.message}"]
        if self.hint:
            parts.append(f"💡 Hint: {self.hint}")
        if self.docs_url:
            parts.append(f"📚 Docs: {self.docs_url}")
        return "\n".join(parts)


def append_to_file(path_env: str, content: str) -> None:
    """Append content to a GitHub Actions environment file."""
    file_path_str = os.environ.get(path_env)
    if not file_path_str:
        raise WorkflowError(
            f"Environment variable {path_env} not set",
            hint="This helper must run inside a GitHub Actions workflow",
            docs_url=(
                "https://docs.github.com/en/actions/using-workflows/"
                "workflow-commands-for-github-actions"
            ),
        )

    file_path = Path(file_path_str)
    if not file_path.exists():
        raise WorkflowError(
            f"File {file_path} does not exist",
            hint=f"Ensure GitHub Actions created the {path_env} file",
        )

    with file_path.open("a", encoding="utf-8") as handle:
        handle.write(content)


def write_output(name: str, value: str) -> None:
    """Write an output variable for downstream workflow steps."""
    append_to_file("GITHUB_OUTPUT", f"{name}={value}\n")


def append_env(name: str, value: str) -> None:
    """Append an environment variable for later workflow steps."""
    append_to_file("GITHUB_ENV", f"{name}={value}\n")


def append_summary(text: str) -> None:
    """Append markdown content to the GitHub Actions step summary."""
    append_to_file("GITHUB_STEP_SUMMARY", text)


def get_repository_config() -> dict[str, Any]:
    """Load and cache `.github/repository-config.yml`."""
    global _CONFIG_CACHE

    if _CONFIG_CACHE is not None:
        return _CONFIG_CACHE

    config_path = Path(".github/repository-config.yml")
    if not config_path.exists():
        raise WorkflowError(
            "repository-config.yml not found",
            hint=(
                "Run: cp .github/repository-config.example.yml "
                ".github/repository-config.yml"
            ),
            docs_url=(
                "https://github.com/jdfalk/ghcommon/"
                "docs/refactors/workflows/v2/reference/config-schema.md"
            ),
        )

    try:
        with config_path.open(encoding="utf-8") as handle:
            _CONFIG_CACHE = yaml.safe_load(handle)
    except yaml.YAMLError as error:
        raise WorkflowError(
            f"Invalid YAML in repository-config.yml: {error}",
            hint="Validate with: yamllint .github/repository-config.yml",
        ) from error

    if not isinstance(_CONFIG_CACHE, dict):
        raise WorkflowError(
            "repository-config.yml must contain a YAML dictionary",
            hint="Ensure the file starts with top-level keys",
        )

    return _CONFIG_CACHE


def config_path(default: Any, *path: str) -> Any:
    """Navigate configuration dictionary and return value or default."""
    current: Any = get_repository_config()
    for key in path:
        if not isinstance(current, dict) or key not in current:
            return default
        current = current[key]
    return current


@contextmanager
def timed_operation(operation_name: str):
    """Context manager that records duration for an operation."""
    start_time = time.time()
    try:
        yield
    finally:
        duration = time.time() - start_time
        print(f"⏱️  {operation_name} took {duration:.2f}s")
        try:
            append_summary(f"| {operation_name} | {duration:.2f}s |\n")
        except WorkflowError as error:
            print(sanitize_log(str(error)), file=sys.stderr)


def handle_error(error: Exception, context: str) -> None:
    """Handle workflow errors by printing details and exiting."""
    message = str(error)
    if isinstance(error, WorkflowError):
        message = str(error)
    else:
        message = f"❌ Unexpected error in {context}: {error}"
    print(sanitize_log(message), file=sys.stderr)
    sys.exit(1)


def sanitize_log(message: str) -> str:
    """Mask sensitive tokens from log messages."""
    import re

    sanitized = re.sub(r"ghp_[a-zA-Z0-9]{36}", "***GITHUB_TOKEN***", message)
    sanitized = re.sub(r"ghs_[a-zA-Z0-9]{36}", "***GITHUB_SECRET***", sanitized)
    sanitized = re.sub(
        r"Bearer\s+[a-zA-Z0-9\-._~+/]+=*",
        "Bearer ***TOKEN***",
        sanitized,
    )
    return sanitized


def ensure_file(path: Path, content: str) -> bool:
    """Create file with content if it does not already exist."""
    if path.exists():
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    return True


def get_feature_flag(flag_name: str, default: bool = False) -> bool:
    """Return feature flag state from configuration."""
    return bool(config_path(default, "workflows", "experimental", flag_name))


def require_feature_flag(flag_name: str) -> None:
    """Ensure feature flag enabled, otherwise raise WorkflowError."""
    if not get_feature_flag(flag_name):
        raise WorkflowError(
            f"Feature '{flag_name}' not enabled",
            hint=(
                "Enable in repository-config.yml: "
                f"workflows.experimental.{flag_name}: true"
            ),
            docs_url=(
                "https://github.com/jdfalk/ghcommon/"
                "docs/refactors/workflows/v2/architecture.md#migration-strategy"
            ),
        )
