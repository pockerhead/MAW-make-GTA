"""Summary of a Bevy `trace_chrome` file (TRACE_CHROME=<path>, cargo feature `profile`).

The file is a JSON array with one event per line, written by tracing-chrome: `B`/`E` pairs per thread
(`tid`), `ts` in microseconds, span names formatted by bevy_log as `<span>: <fields>`, e.g.
`update`, `schedule: name=FixedMain`, `system: name="gta_sim::gang::behavior::drive_members"`.
Traces reach gigabytes at high FPS: the file is streamed line by line from a byte offset found by
bisection on `ts` (events are written in time order, give or take thread jitter, so reading starts
`SEEK_MARGIN_S` before the window), never `json.load`-ed, and a truncated last line (a game killed
before its FlushGuard dropped) is skipped.

Run: python tools/qa/trace.py <trace.json> [window_s]
"""

import json
from pathlib import Path
import re
import sys

REPO = Path(__file__).resolve().parents[2]
TAIL_BYTES = 64 * 1024
SEEK_MARGIN_S = 1.0
# Bisection stops once the bracket is this small; the reader then scans it.
BISECT_BYTES = 1 << 20
# gta_sim modules that are not AI: player/combat/vehicle/world state, config and plumbing (TASK-015).
NON_AI = {"character", "combat", "config", "flow", "layers", "player", "vehicle", "world"}
PHYSICS_PREFIXES = ("avian3d::", "bevy_tnua")
# Exclusive systems that run a whole schedule (fixed main, physics, substeps, render).
RUNNER = re.compile(r"::run_\w*schedule$")


def sim_modules(lib=REPO / "crates" / "gta_sim" / "src" / "lib.rs"):
    """Top-level modules of gta_sim, read from its lib.rs."""
    text = lib.read_text(encoding="utf-8")
    return set(re.findall(r"^\s*pub(?:\(crate\))?\s+mod\s+(\w+)\s*;", text, re.M))


def parse(line):
    line = line.strip().lstrip(",").rstrip(",").strip()
    if not line or line in ("[", "]"):
        return None
    try:
        return json.loads(line)
    except json.JSONDecodeError:
        return None


def ts_at(f, offset):
    """`ts` of the first timestamped event starting after byte `offset` (None at the end)."""
    f.seek(offset)
    if offset:
        f.readline()
    for _ in range(64):
        raw = f.readline()
        if not raw:
            return None
        event = parse(raw.decode("utf-8", errors="replace"))
        if event and "ts" in event:
            return float(event["ts"])
    return None


def window_offset(path, start):
    """A byte offset whose events all have `ts` < `start` - SEEK_MARGIN_S (0 for small files)."""
    target = start - SEEK_MARGIN_S * 1e6
    lo, hi = 0, Path(path).stat().st_size
    with open(path, "rb") as f:
        while hi - lo > BISECT_BYTES:
            mid = (lo + hi) // 2
            ts = ts_at(f, mid)
            if ts is None or ts >= target:
                hi = mid
            else:
                lo = mid
    return lo


def events(path, offset=0):
    """Parsed events of the trace from byte `offset`, in file order; unparsable lines (a line cut by
    the offset, the truncated tail) are skipped."""
    with open(path, "rb") as f:
        f.seek(offset)
        for raw in f:
            event = parse(raw.decode("utf-8", errors="replace"))
            if event is not None:
                yield event


def max_ts(path):
    """Largest `ts` among the events of the file's last ~64 KB."""
    size = Path(path).stat().st_size
    with open(path, "rb") as f:
        f.seek(max(0, size - TAIL_BYTES))
        tail = f.read().decode("utf-8", errors="replace")
    stamps = [float(m) for m in re.findall(r'"ts":\s*([0-9.eE+-]+)', tail)]
    if not stamps:
        raise ValueError(f"{path}: no timestamped event in the last {TAIL_BYTES} bytes")
    return max(stamps)


def span_kind(name):
    """(`kind`, `field value`) of a bevy span name: ("system", "gta_sim::x::y"), ("schedule",
    "FixedMain"), ("update", None), or (name, None) for anything else."""
    head, _, fields = name.partition(":")
    head = head.strip()
    match = re.search(r'name=("?)(.*?)\1\s*$', fields.strip())
    return head, (match.group(2) if match else None)


def stats(values):
    if not values:
        return {"count": 0}
    ordered = sorted(values)

    def pct(p):
        return ordered[min(len(ordered) - 1, int(p / 100.0 * len(ordered)))]

    return {
        "count": len(values),
        "mean_ms": sum(values) / len(values),
        "p50_ms": pct(50),
        "p99_ms": pct(99),
        "max_ms": ordered[-1],
    }


def classify(system, modules):
    """Share of a system: "runner" (an exclusive system that runs a schedule: its own time is waiting
    for the systems it runs), "physics", "ai", "unclassified" (a gta_sim module lib.rs does not list)
    or "other"."""
    if RUNNER.search(system):
        return "runner"
    if system.startswith(PHYSICS_PREFIXES):
        return "physics"
    if not system.startswith("gta_sim::"):
        return "other"
    module = system.split("::")[1]
    if module not in modules:
        return "unclassified"
    return "other" if module in NON_AI else "ai"


def summarize(path, window_s=10.0, modules=None):
    """Frames, FixedMain ticks and systems of the last `window_s` seconds of the trace. System times
    are exclusive: a span nested in another on the same thread (a schedule runner and the systems it
    runs inline) is taken out of its parent, so shares never count a millisecond twice."""
    modules = sim_modules() if modules is None else modules
    start = max_ts(path) - window_s * 1e6
    stacks = {}
    frames, fixed = [], []
    systems = {}
    for e in events(path, window_offset(path, start)):
        ph, tid = e.get("ph"), e.get("tid")
        if ph == "B":
            stacks.setdefault(tid, []).append([e.get("name", ""), float(e["ts"]), 0.0])
            continue
        if ph != "E":
            continue
        stack = stacks.get(tid)
        name = e.get("name", "")
        # Pop to the matching begin: an unmatched begin (span cut by the trace start) is dropped.
        while stack and stack[-1][0] != name:
            stack.pop()
        if not stack:
            continue
        _, begin, nested = stack.pop()
        ms = (float(e["ts"]) - begin) / 1000.0
        if stack:
            stack[-1][2] += ms
        if begin < start:
            continue
        kind, value = span_kind(name)
        if kind == "update":
            frames.append(ms)
        elif kind == "schedule" and value == "FixedMain":
            fixed.append(ms)
        elif kind == "system" and value:
            systems[value] = systems.get(value, 0.0) + ms - nested
    frame_count = max(len(frames), 1)
    tick_count = max(len(fixed), 1)
    shares = {"ai": 0.0, "physics": 0.0, "unclassified": 0.0}
    unclassified = set()
    for system, total in systems.items():
        group = classify(system, modules)
        if group in shares:
            shares[group] += total
        if group == "unclassified":
            unclassified.add(system.split("::")[1])
    top = sorted(systems.items(), key=lambda kv: -kv[1])
    return {
        "window_s": window_s,
        "frames": stats(frames),
        "fixed_main": stats(fixed),
        "systems_ms_per_frame": [(name, total / frame_count) for name, total in top],
        "systems_ms_total": dict(top),
        "ms_per_tick": {group: total / tick_count for group, total in shares.items()},
        "unclassified_modules": sorted(unclassified),
    }


if __name__ == "__main__":
    summary = summarize(sys.argv[1], float(sys.argv[2]) if len(sys.argv) > 2 else 10.0)
    summary["systems_ms_per_frame"] = summary["systems_ms_per_frame"][:15]
    summary.pop("systems_ms_total")
    print(json.dumps(summary, indent=2))
