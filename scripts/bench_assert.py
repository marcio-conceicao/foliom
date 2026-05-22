#!/usr/bin/env python3
"""CI gate parser for Criterion's estimates.json (plan 02-08, D-35).

Usage:
    python3 scripts/bench_assert.py <path/to/estimates.json> <ceiling_ns>

Reads the ``mean.point_estimate`` field from Criterion's output and exits
with status 0 if the measured mean is strictly less than the ceiling,
status 1 otherwise. Prints a single human-readable comparison line.

ACPT-02 ceiling for the cold-start bench is ``3_000_000_000`` ns
(target 2s × 1.5 per D-35 — absorbs CI runner variance vs the M1-class
reference laptop per Assumption A8).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("usage: bench_assert.py <estimates.json> <ceiling_ns>",
              file=sys.stderr)
        return 2

    estimates_path = Path(argv[1])
    try:
        ceiling_ns = int(argv[2])
    except ValueError:
        print(f"ceiling_ns must be integer, got {argv[2]!r}", file=sys.stderr)
        return 2

    if not estimates_path.is_file():
        print(f"estimates file not found: {estimates_path}", file=sys.stderr)
        return 2

    with estimates_path.open() as fh:
        data = json.load(fh)

    try:
        mean_ns = float(data["mean"]["point_estimate"])
    except (KeyError, TypeError, ValueError) as exc:
        print(f"malformed Criterion estimates.json: {exc}", file=sys.stderr)
        return 2

    verdict = "PASS" if mean_ns < ceiling_ns else "FAIL"
    print(f"{verdict} mean={mean_ns / 1e9:.3f}s "
          f"ceiling={ceiling_ns / 1e9:.3f}s "
          f"({estimates_path})")
    return 0 if mean_ns < ceiling_ns else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
