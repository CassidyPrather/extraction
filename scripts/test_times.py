#!/usr/bin/env python3
"""Times every lib test and fails any that has grown past its budget.

    scripts/test_times.py                 # build, run the suite, check
    scripts/test_times.py --threads 2     # as many test threads as that
    scripts/test_times.py --json FILE     # check a run already recorded
    scripts/test_times.py --budget 90     # seconds a test may take
    scripts/test_times.py -- --skip render::budget   # libtest's own filters

Runs the lib test binary with libtest's own timing (`--report-time`, its
JSON format; RUSTC_BOOTSTRAP=1 is set for the test process only, so the
build is the ordinary one), prints the slowest tests and every one over
budget, and exits 1 if any is. Times are wall time under the suite's own
load, four tests at a time, not alone.

A test that is slow on purpose is listed in scripts/slow_tests.txt with
its own budget and why. A gauge too heavy for the suite is `#[ignore]`d
instead (the render gauges: `cargo test --lib -- --ignored gauge_`).
"""

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SLOW = ROOT / "scripts" / "slow_tests.txt"


def slow_list():
    """name -> (budget seconds, why), from scripts/slow_tests.txt."""
    out = {}
    if not SLOW.exists():
        return out
    for line in SLOW.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        name, budget, why = (line.split(None, 2) + ["", ""])[:3]
        out[name] = (float(budget), why)
    return out


def test_binary():
    """Builds the lib tests and returns the binary's path."""
    out = subprocess.run(
        ["cargo", "test", "--lib", "--no-run", "--message-format=json"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    for line in out.splitlines():
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        if msg.get("reason") == "compiler-artifact" and msg.get("executable"):
            if msg["target"]["kind"] == ["lib"]:
                return msg["executable"]
    sys.exit("no lib test binary in cargo's output")


def run(threads, record, extra):
    env = dict(os.environ, RUSTC_BOOTSTRAP="1", RUST_LOG="off")
    with open(record, "w") as f:
        subprocess.run(
            [
                test_binary(),
                f"--test-threads={threads}",
                "-Z",
                "unstable-options",
                "--format",
                "json",
                "--report-time",
                *extra,
            ],
            cwd=ROOT,
            env=env,
            stdout=f,
            stderr=subprocess.DEVNULL,
        )


def times(record):
    """(seconds, name, event) for every test that ran."""
    out = []
    for line in Path(record).read_text().splitlines():
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        if e.get("type") == "test" and e.get("event") in ("ok", "failed"):
            out.append((e.get("exec_time", 0.0), e["name"], e["event"]))
    return sorted(out, reverse=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", help="a recorded run to check instead of running")
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--budget", type=float, default=120.0)
    ap.add_argument("--top", type=int, default=20)
    ap.add_argument("libtest", nargs="*", help="after --: passed to the test binary")
    args = ap.parse_args()

    record = args.json
    if not record:
        record = ROOT / "target" / "test_times.json"
        run(args.threads, record, args.libtest)
    ran = times(record)
    slow = slow_list()

    total = sum(t for t, _, _ in ran)
    print(f"{len(ran)} tests, {total:.0f} test-seconds at {args.threads} threads")
    for t, name, event in ran[: args.top]:
        mark = " (FAILED)" if event == "failed" else ""
        print(f"{t:8.1f}  {name}{mark}")

    over = []
    for t, name, _ in ran:
        budget, why = slow.get(name, (args.budget, ""))
        if t > budget:
            over.append((t, name, budget, why))
    stale = sorted(set(slow) - {name for _, name, _ in ran})
    if stale:
        print("\nlisted in slow_tests.txt but not run (renamed? gone?):")
        for name in stale:
            print(f"  {name}")
    if over:
        print(f"\n{len(over)} over budget:")
        for t, name, budget, why in over:
            note = f" (listed: {why})" if why else ""
            print(f"  {t:7.1f} s > {budget:.0f} s  {name}{note}")
        print(
            "\nCut the test (fewer frames, a small arena instead of a whole "
            "valley), or list it in scripts/slow_tests.txt with a budget and why."
        )
        sys.exit(1)
    print(f"\nevery test within budget ({args.budget:.0f} s, or its own)")


if __name__ == "__main__":
    main()
