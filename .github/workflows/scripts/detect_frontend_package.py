#!/usr/bin/env python3
"""Detect frontend package metadata for release workflow."""

from __future__ import annotations

import json
import os
from pathlib import Path

CANDIDATES = [Path("package.json"), *sorted(Path(".").glob("*/package.json"))]


def main() -> None:
    package_path: Path | None = None
    package_name = ""
    package_version = ""
    package_manager = "npm"

    for candidate in CANDIDATES:
        if not candidate.exists():
            continue
        try:
            data = json.loads(candidate.read_text("utf-8"))
        except Exception:
            continue
        name = data.get("name")
        version = data.get("version")
        if name:
            package_name = str(name)
            package_version = str(version or "")
            package_path = candidate.parent
            lock_files = {
                "pnpm-lock.yaml": "pnpm",
                "yarn.lock": "yarn",
                "package-lock.json": "npm",
            }
            for filename, manager in lock_files.items():
                if (candidate.parent / filename).exists():
                    package_manager = manager
                    break
            break

    outputs = {
        "has-package": "true" if package_name else "false",
        "package-dir": package_path.as_posix() if package_path else ".",
        "package-name": package_name,
        "package-version": package_version,
        "package-manager": package_manager,
    }

    for key, value in outputs.items():
        print(f"{key}={value}")

    github_output = os.environ.get("GITHUB_OUTPUT")
    if github_output:
        with open(github_output, "a", encoding="utf-8") as handle:
            for key, value in outputs.items():
                handle.write(f"{key}={value}\n")


if __name__ == "__main__":
    main()
