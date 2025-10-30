#!/usr/bin/env python3
# file: .github/workflows/scripts/release_workflow.py
# version: 1.0.0
# guid: e5f6a7b8-c9d0-1e2f-3a4b5c6d7e8f9a0b

"""Release workflow helper functions for version management and publishing."""

from __future__ import annotations

import re
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

import workflow_common


@dataclass
class ReleaseInfo:
    """Release information including version, branch, and language context."""

    version: str
    branch: str
    language: str
    language_version: Optional[str] = None
    tag: str = ""
    is_stable_branch: bool = False

    def __post_init__(self) -> None:
        """Generate release tag based on branch context."""
        if self.language_version and self.is_stable_branch:
            clean_version = self.language_version.replace(".", "")
            self.tag = f"v{self.version}-{self.language}{clean_version}"
        else:
            self.tag = f"v{self.version}"


def detect_primary_language() -> str:
    """Detect primary programming language from repository structure."""
    repo_root = Path.cwd()
    patterns = [
        ("go", ["go.mod", "go.sum", "main.go"]),
        ("rust", ["Cargo.toml", "Cargo.lock", "src/main.rs"]),
        ("python", ["setup.py", "pyproject.toml", "requirements.txt"]),
        ("node", ["package.json", "package-lock.json"]),
    ]

    for language, files in patterns:
        for file_name in files:
            if (repo_root / file_name).exists():
                return language

    raise workflow_common.WorkflowError(
        "Could not detect primary language",
        hint=(
            "Ensure repository contains language files such as go.mod or "
            "Cargo.toml"
        ),
    )


def get_branch_language_version(
    branch_name: str,
    language: str,
) -> Optional[str]:
    """Extract language version from stable branch name."""
    if branch_name in {"main", "master", "develop"}:
        return None

    pattern = rf"stable-1-{language}-(.+)$"
    match = re.match(pattern, branch_name)
    if match:
        return match.group(1)

    return None


def is_stable_branch(branch_name: str) -> bool:
    """Return True if branch follows stable-1-* convention."""
    return branch_name.startswith("stable-1-")


def get_current_branch() -> str:
    """Return the current git branch."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            capture_output=True,
            text=True,
            check=True,
        )
        return result.stdout.strip()
    except subprocess.CalledProcessError as error:
        raise workflow_common.WorkflowError(
            f"Failed to get current branch: {error}",
            hint="Ensure git repository is initialized",
        ) from error


def extract_version_from_file(language: str) -> str:
    """Extract semantic version from language-specific manifest."""
    version_patterns = {
        "go": ("go.mod", r"// v([0-9]+\.[0-9]+\.[0-9]+)"),
        "rust": ("Cargo.toml", r'version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"'),
        "python": (
            "pyproject.toml",
            r'version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"',
        ),
        "node": ("package.json", r'"version"\s*:\s*"([0-9]+\.[0-9]+\.[0-9]+)"'),
    }

    if language not in version_patterns:
        raise workflow_common.WorkflowError(
            f"Unsupported language: {language}",
            hint=f"Supported languages: {', '.join(version_patterns.keys())}",
        )

    file_name, pattern = version_patterns[language]
    manifest = Path(file_name)

    if not manifest.exists():
        raise workflow_common.WorkflowError(
            f"Version file not found: {file_name}",
            hint=f"Create {file_name} with semantic version for {language}",
        )

    content = manifest.read_text(encoding="utf-8")
    match = re.search(pattern, content)
    if not match:
        raise workflow_common.WorkflowError(
            f"Version not found in {file_name}",
            hint=f"Ensure {file_name} contains version in semantic format",
        )

    return match.group(1)


def create_release_info(
    version: Optional[str] = None,
    branch: Optional[str] = None,
) -> ReleaseInfo:
    """Create comprehensive release information object."""
    language = detect_primary_language()
    resolved_branch = branch or get_current_branch()
    resolved_version = version or extract_version_from_file(language)
    stable = is_stable_branch(resolved_branch)
    language_version = None
    if stable:
        language_version = get_branch_language_version(
            resolved_branch,
            language,
        )

    return ReleaseInfo(
        version=resolved_version,
        branch=resolved_branch,
        language=language,
        language_version=language_version,
        is_stable_branch=stable,
    )


def generate_changelog(
    previous_tag: Optional[str] = None,
    current_tag: Optional[str] = None,
) -> str:
    """Generate changelog from git commits since last release."""
    if previous_tag is None:
        try:
            result = subprocess.run(
                ["git", "describe", "--tags", "--abbrev=0", "HEAD^"],
                capture_output=True,
                text=True,
                check=True,
            )
            previous_tag = result.stdout.strip()
        except subprocess.CalledProcessError:
            previous_tag = ""

    git_range = current_tag or "HEAD"
    args = ["git", "log", "--pretty=format:%s"]
    if previous_tag:
        args.insert(2, f"{previous_tag}..{git_range}")
    else:
        args.insert(2, git_range)

    try:
        result = subprocess.run(
            args,
            capture_output=True,
            text=True,
            check=True,
        )
        commits = [line for line in result.stdout.strip().split("\n") if line]
    except subprocess.CalledProcessError as error:
        raise workflow_common.WorkflowError(
            f"Failed to generate changelog: {error}",
            hint="Ensure git history is available for changelog generation",
        ) from error

    features: list[str] = []
    fixes: list[str] = []
    breaking: list[str] = []
    other: list[str] = []

    for commit in commits:
        normalized = commit.strip()
        if not normalized:
            continue
        prefix = normalized.split(":", 1)[0]
        if "BREAKING CHANGE" in normalized or "!" in prefix or normalized.startswith(
            "!"
        ):
            breaking.append(normalized)
        elif normalized.startswith("feat"):
            features.append(normalized)
        elif normalized.startswith("fix"):
            fixes.append(normalized)
        else:
            other.append(normalized)

    sections: list[str] = []

    if breaking:
        sections.append("## ⚠️ Breaking Changes\n")
        sections.extend(f"- {message}" for message in breaking)
        sections.append("")

    if features:
        sections.append("## ✨ Features\n")
        sections.extend(f"- {message}" for message in features)
        sections.append("")

    if fixes:
        sections.append("## 🐛 Bug Fixes\n")
        sections.extend(f"- {message}" for message in fixes)
        sections.append("")

    if other:
        sections.append("## 📝 Other Changes\n")
        sections.extend(f"- {message}" for message in other)
        sections.append("")

    if not sections:
        return "## 📝 Other Changes\n\n- No notable changes"

    return "\n".join(sections)


def main() -> None:
    """Entry point for CLI usage."""
    import argparse

    parser = argparse.ArgumentParser(
        description="Generate release information and changelog",
    )
    parser.add_argument("--version", help="Version override (semantic version)")
    parser.add_argument("--branch", help="Branch name override")
    parser.add_argument(
        "--output",
        action="store_true",
        help="Write release information to GITHUB_OUTPUT",
    )
    parser.add_argument(
        "--generate-changelog",
        action="store_true",
        help="Generate changelog from git history",
    )

    args = parser.parse_args()

    try:
        with workflow_common.timed_operation("Generate release info"):
            info = create_release_info(
                version=args.version,
                branch=args.branch,
            )

        print("📦 Release Information:")
        print(f"   Language: {info.language}")
        print(f"   Version: {info.version}")
        print(f"   Branch: {info.branch}")
        print(f"   Tag: {info.tag}")
        print(f"   Stable Branch: {'✅' if info.is_stable_branch else '❌'}")
        if info.language_version:
            print(f"   Language Version: {info.language_version}")

        if args.output:
            workflow_common.write_output("language", info.language)
            workflow_common.write_output("version", info.version)
            workflow_common.write_output("tag", info.tag)
            workflow_common.write_output("branch", info.branch)
            workflow_common.write_output(
                "is_stable", str(info.is_stable_branch).lower()
            )
            if info.language_version:
                workflow_common.write_output(
                    "language_version",
                    info.language_version,
                )

        if args.generate_changelog:
            with workflow_common.timed_operation("Generate changelog"):
                changelog = generate_changelog(current_tag=info.tag)

            print("\n📝 Changelog:")
            print(changelog)

            if args.output:
                escaped = (
                    changelog.replace("%", "%25")
                    .replace("\n", "%0A")
                    .replace("\r", "%0D")
                )
                workflow_common.write_output("changelog", escaped)

            try:
                summary_body = f"## Release {info.tag}\n\n{changelog}"
                workflow_common.append_summary(summary_body)
            except workflow_common.WorkflowError as error:
                print(workflow_common.sanitize_log(str(error)))

    except Exception as error:  # pylint: disable=broad-except
        workflow_common.handle_error(error, "Release preparation")


if __name__ == "__main__":
    main()
