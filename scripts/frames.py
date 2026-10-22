#!/usr/bin/env python3
"""Worst frame and 99.9th percentile, CPU and GPU, from EXTRACTION_FRAMELOG.

    scripts/frames.py frames.log [--skip N]

--skip drops the first N frames (loading). See docs/PERFORMANCE.md,
"Hitches".
"""
import sys


def pct(v, q):
    v = sorted(v)
    return v[min(len(v) - 1, int(len(v) * q))] if v else float("nan")


def main():
    args = sys.argv[1:]
    skip = 0
    if "--skip" in args:
        i = args.index("--skip")
        skip = int(args[i + 1])
        del args[i : i + 2]
    cpu, gpu = [], []
    worst_cpu = worst_gpu = (0.0, 0)
    for line in open(args[0]):
        if line.startswith("#"):
            continue
        f, c, g = line.split()[:3]
        f, c, g = int(f), float(c), float(g)
        if f <= skip:
            continue
        cpu.append(c)
        worst_cpu = max(worst_cpu, (c, f))
        if g >= 0:
            gpu.append(g)
            worst_gpu = max(worst_gpu, (g, f))
    for name, v, w in (("cpu", cpu, worst_cpu), ("gpu", gpu, worst_gpu)):
        if not v:
            print(f"{name}: no frames")
            continue
        print(
            f"{name}: {len(v)} frames, p50 {pct(v, 0.5):.1f} ms, p99 {pct(v, 0.99):.1f}, "
            f"p99.9 {pct(v, 0.999):.1f}, worst {w[0]:.1f} at frame {w[1]}"
        )


if __name__ == "__main__":
    main()
