#!/usr/bin/env python3
# file: tests/workflow_scripts/__init__.py
# version: 1.0.0
# guid: c3d4e5f6-a7b8-9c0d-1e2f-3a4b5c6d7e8f

"""Test package configuration for workflow helper scripts."""

from __future__ import annotations

from pathlib import Path
import sys

SCRIPTS_PATH = Path(__file__).resolve().parents[2] / ".github/workflows/scripts"
if str(SCRIPTS_PATH) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_PATH))
