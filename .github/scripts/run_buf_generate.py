#!/usr/bin/env python3
# file: .github/scripts/run_buf_generate.py
# version: 1.0.0
# guid: c4d2e6f3-5b7a-49d1-8e2f-6a1b9c7d8e20
"""Run buf generate with basic safety checks and clearer logging."""

from __future__ import annotations
import os
import subprocess
import sys


def main() -> int:
    if not os.path.exists("buf.gen.yaml"):
        print("No buf.gen.yaml found, skipping generation")
        return 0
    print("Running buf generate...")
    proc = subprocess.run(["buf", "generate"], text=True)
    if proc.returncode != 0:
        print("buf generate failed", file=sys.stderr)
        return proc.returncode
    print("buf generate completed successfully")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
