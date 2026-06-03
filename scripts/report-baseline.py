#!/usr/bin/env python3
"""
Report baseline reliability statistics for a git-perf benchmark measurement.

Usage:
    python3 scripts/report-baseline.py [--commit COMMIT] [--baseline-only] [--measurement NAME]

Reads git-notes from refs/notes/perf-v3 for the given commit (default: HEAD),
parses the \\x1e-delimited serialization format, and reports statistics on the
filtered measurements.

Requires: Python >= 3.8, no third-party dependencies.
Run 'git perf pull' before this script to ensure remote measurements are present.
"""

import argparse
import math
import subprocess
import sys

NOTES_REF = "refs/notes/perf-v3"
DEFAULT_MEASUREMENT = "bench::add_measurements/add_measurement/1::median"
DELIMITER = "\x1e"


def resolve_commit(committish: str) -> str:
    result = subprocess.run(
        ["git", "rev-parse", committish],
        capture_output=True, text=True, check=True,
    )
    return result.stdout.strip()


def get_note(commit_sha: str) -> str:
    result = subprocess.run(
        ["git", "notes", "--ref", NOTES_REF, "show", commit_sha],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        return ""
    return result.stdout


def parse_records(note_text: str):
    records = []
    for line in note_text.splitlines():
        line = line.strip()
        if not line:
            continue
        components = [c for c in line.split(DELIMITER) if c]
        if len(components) < 4:
            continue
        try:
            epoch = int(components[0])
        except ValueError:
            continue
        name = components[1]
        try:
            timestamp = float(components[2])
        except ValueError:
            continue
        try:
            value = float(components[3])
        except ValueError:
            continue
        key_values = {}
        for kv in components[4:]:
            if "=" in kv:
                k, v = kv.split("=", 1)
                key_values[k] = v
        records.append({
            "epoch": epoch,
            "name": name,
            "timestamp": timestamp,
            "value": value,
            "key_values": key_values,
        })
    return records


def compute_stats(values):
    n = len(values)
    if n == 0:
        return {}
    sorted_vals = sorted(values)
    mean = sum(values) / n
    if n % 2 == 1:
        median = sorted_vals[n // 2]
    else:
        median = (sorted_vals[n // 2 - 1] + sorted_vals[n // 2]) / 2.0
    if n > 1:
        variance = sum((x - mean) ** 2 for x in values) / (n - 1)
        stddev = math.sqrt(variance)
    else:
        stddev = 0.0
    cov = (stddev / mean * 100.0) if mean != 0 else float("nan")
    return {
        "count": n,
        "min": min(values),
        "max": max(values),
        "mean": mean,
        "median": median,
        "stddev": stddev,
        "cov_pct": cov,
    }


def ns_to_human(ns: float) -> str:
    if ns >= 1_000_000_000:
        return f"{ns / 1_000_000_000:.3f} s"
    if ns >= 1_000_000:
        return f"{ns / 1_000_000:.3f} ms"
    if ns >= 1_000:
        return f"{ns / 1_000:.3f} µs"
    return f"{ns:.1f} ns"


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--commit", default="HEAD",
        help="Commit to read notes from (default: HEAD)",
    )
    parser.add_argument(
        "--baseline-only", action="store_true",
        help="Only include records tagged baseline=true",
    )
    parser.add_argument(
        "--measurement", default=DEFAULT_MEASUREMENT,
        help=f"Measurement name to filter on (default: {DEFAULT_MEASUREMENT})",
    )
    args = parser.parse_args()

    try:
        commit_sha = resolve_commit(args.commit)
    except subprocess.CalledProcessError as e:
        print(f"Error: cannot resolve commit '{args.commit}': {e.stderr.strip()}", file=sys.stderr)
        sys.exit(1)

    note_text = get_note(commit_sha)
    if not note_text:
        print(f"No git-notes found for commit {commit_sha[:12]} in {NOTES_REF}.")
        print("Did you run 'git perf pull' first?")
        sys.exit(1)

    records = parse_records(note_text)
    filtered = [
        r for r in records
        if r["name"] == args.measurement
        and (not args.baseline_only or r["key_values"].get("baseline") == "true")
    ]

    if not filtered:
        qualifiers = []
        if args.baseline_only:
            qualifiers.append("baseline=true")
        qualifier_str = f" with {', '.join(qualifiers)}" if qualifiers else ""
        print(f"No records found for measurement '{args.measurement}'{qualifier_str}.")
        print(f"Total records in note: {len(records)}")
        sys.exit(1)

    values = [r["value"] for r in filtered]
    stats = compute_stats(values)

    print(f"Measurement : {args.measurement}")
    print(f"Commit      : {commit_sha[:12]}")
    print(f"Filter      : {'baseline=true only' if args.baseline_only else 'all matching records'}")
    print()
    print(f"Count  : {stats['count']}")
    print(f"Min    : {ns_to_human(stats['min'])} ({stats['min']:.1f} ns)")
    print(f"Max    : {ns_to_human(stats['max'])} ({stats['max']:.1f} ns)")
    print(f"Mean   : {ns_to_human(stats['mean'])} ({stats['mean']:.1f} ns)")
    print(f"Median : {ns_to_human(stats['median'])} ({stats['median']:.1f} ns)")
    print(f"StdDev : {ns_to_human(stats['stddev'])} ({stats['stddev']:.1f} ns)")
    print(f"CoV    : {stats['cov_pct']:.2f}%")
    print()
    print("Raw values (ns), ordered by timestamp:")
    for i, r in enumerate(sorted(filtered, key=lambda x: x["timestamp"]), 1):
        kv_str = " ".join(f"{k}={v}" for k, v in sorted(r["key_values"].items()))
        print(f"  [{i:02d}] {r['value']:.1f} ns  |  {kv_str}")


if __name__ == "__main__":
    main()
