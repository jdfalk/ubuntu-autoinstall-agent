#!/usr/bin/env python3
# file: .github/workflows/scripts/validate_config.py
# version: 1.0.0
# guid: f6a7b8c9-d0e1-2f3a-4b5c-6d7e8f9a0b1c

"""Validate repository-config.yml against JSON schema.

Usage:
    python validate_config.py [--schema PATH] [--config PATH]
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

import workflow_common
import yaml

try:
    from jsonschema import ValidationError, validate
except ImportError:  # pragma: no cover - smoke test via handle_error
    workflow_common.handle_error(
        workflow_common.WorkflowError(
            "jsonschema package not installed",
            hint="Install dependency with: pip install jsonschema pyyaml",
        ),
        "import jsonschema",
    )


def validate_repository_config(
    schema_path: Path,
    config_path: Path,
) -> bool:
    """Validate config file against schema."""
    try:
        with schema_path.open(encoding="utf-8") as handle:
            schema = json.load(handle)
    except Exception as error:  # pylint: disable=broad-except
        message = f"❌ Failed to load schema from {schema_path}: {error}"
        print(workflow_common.sanitize_log(message))
        return False

    try:
        with config_path.open(encoding="utf-8") as handle:
            config = yaml.safe_load(handle)
    except Exception as error:  # pylint: disable=broad-except
        message = f"❌ Failed to load config from {config_path}: {error}"
        print(workflow_common.sanitize_log(message))
        return False

    try:
        validate(config, schema)
        print(f"✅ Configuration valid: {config_path}")
        return True
    except ValidationError as error:
        print(f"❌ Configuration invalid: {config_path}")
        print(f"   Error: {error.message}")
        path = ".".join(str(part) for part in error.path)
        if path:
            print(f"   Path: {path}")
        return False


def main() -> None:
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Validate repository-config.yml",
    )
    parser.add_argument(
        "--schema",
        type=Path,
        default=Path(".github/schemas/repository-config.schema.json"),
        help="Path to JSON schema",
    )
    parser.add_argument(
        "--config",
        type=Path,
        default=Path(".github/repository-config.yml"),
        help="Path to repository config",
    )

    args = parser.parse_args()

    if not args.schema.exists():
        print(f"❌ Schema not found: {args.schema}")
        sys.exit(1)

    if not args.config.exists():
        print(f"❌ Config not found: {args.config}")
        sys.exit(1)

    if not validate_repository_config(args.schema, args.config):
        sys.exit(1)


if __name__ == "__main__":
    main()
