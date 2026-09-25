"""Runs one runtime scenario N times in a row and reports every run (flakes are counted, never hidden
behind one green run).

Run: python tools/qa/repeat.py t9 --runs 20 --out target/qa/rep
Each run k writes to <out>/<name>/run<k> (its own --out) and <out>/<name>/run<k>.log; exits 1 if any
run failed.
"""

import argparse
from pathlib import Path
import subprocess
import sys
import time

REPO = Path(__file__).resolve().parents[2]


def first_error(log):
    lines = log.splitlines()
    for line in lines:
        if "AssertionError" in line or "Error:" in line:
            return line.strip()
    return lines[-1].strip() if lines else "(no output)"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("name", help="scenario name, e.g. t9")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "rep")
    args = parser.parse_args()
    script = REPO / "tools" / "qa" / "scenarios" / f"{args.name}.py"
    if not script.exists():
        sys.exit(f"no scenario {script}")
    root = args.out / args.name
    root.mkdir(parents=True, exist_ok=True)
    failed = 0
    for k in range(1, args.runs + 1):
        started = time.monotonic()
        result = subprocess.run(
            [sys.executable, str(script), "--out", str(root / f"run{k}")],
            cwd=REPO, capture_output=True, text=True, encoding="utf-8", errors="replace",
        )
        log = result.stdout + result.stderr
        (root / f"run{k}.log").write_text(log, encoding="utf-8")
        seconds = time.monotonic() - started
        if result.returncode == 0:
            print(f"{args.name} run {k}/{args.runs}: pass ({seconds:.0f} s)", flush=True)
        else:
            failed += 1
            print(f"{args.name} run {k}/{args.runs}: FAIL ({seconds:.0f} s): {first_error(log)}", flush=True)
    print(f"{args.name}: {args.runs - failed}/{args.runs} passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
