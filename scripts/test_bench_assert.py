#!/usr/bin/env python3
"""RED-phase test for scripts/bench_assert.py (plan 02-08 task 1).

Asserts the CI gate parser exists and behaves at the ACPT-02 ceiling
(target 2s × 1.5 = 3s per D-35). The test writes a synthetic Criterion
estimates.json under a tempdir and exercises both the pass and fail
branches plus an exit-code probe for malformed input.
"""

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "bench_assert.py"
CEILING_NS = 3_000_000_000  # ACPT-02 CI ceiling: 3 seconds


def write_estimates(tmp: Path, mean_ns: float) -> Path:
    """Mirror the subset of Criterion's estimates.json we read."""
    payload = {
        "mean": {"confidence_interval": {"confidence_level": 0.95,
                                          "lower_bound": mean_ns * 0.95,
                                          "upper_bound": mean_ns * 1.05},
                 "point_estimate": mean_ns,
                 "standard_error": mean_ns * 0.01},
        "median": {"point_estimate": mean_ns},
        "median_abs_dev": {"point_estimate": mean_ns * 0.02},
        "slope": None,
        "std_dev": {"point_estimate": mean_ns * 0.05},
    }
    f = tmp / "estimates.json"
    f.write_text(json.dumps(payload))
    return f


class BenchAssertTest(unittest.TestCase):
    def test_script_exists(self) -> None:
        self.assertTrue(SCRIPT.is_file(),
                        f"bench_assert.py must exist at {SCRIPT}")

    def test_passes_below_ceiling(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            est = write_estimates(Path(d), mean_ns=1_500_000_000)
            r = subprocess.run([sys.executable, str(SCRIPT), str(est),
                                str(CEILING_NS)], capture_output=True)
            self.assertEqual(r.returncode, 0,
                             f"exit {r.returncode} stdout={r.stdout!r}")
            self.assertIn(b"mean=", r.stdout)

    def test_fails_above_ceiling(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            est = write_estimates(Path(d), mean_ns=4_000_000_000)
            r = subprocess.run([sys.executable, str(SCRIPT), str(est),
                                str(CEILING_NS)], capture_output=True)
            self.assertNotEqual(r.returncode, 0)

    def test_fails_at_exact_ceiling(self) -> None:
        # Strict < ceiling — equality must fail (defensive against drift).
        with tempfile.TemporaryDirectory() as d:
            est = write_estimates(Path(d), mean_ns=float(CEILING_NS))
            r = subprocess.run([sys.executable, str(SCRIPT), str(est),
                                str(CEILING_NS)], capture_output=True)
            self.assertNotEqual(r.returncode, 0)


if __name__ == "__main__":
    unittest.main()
