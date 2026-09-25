"""Runtime T16: `--bench-scene`, the worst scene of GDD §11, measured (evidence for the owner, FPS is
never pass/fail).

Session A (`--features dev`, release): the bench boards the player into a parked car near the Downtown
centre, pins 5 stars and drives a block loop at 1920x1080. Composition within 120 s: the player drives,
5 stars, active police cars and live units (on foot plus crews aboard) at the escalation row-5 caps
(parsed from escalation.ron),
at least 20 traffic cars; civilians and gang members near the bench centre are recorded as they are (no
scene hacking: the §11 "12 gang members" is a worst case the city rarely puts downtown). Three
screenshots, `Game.frame_report()` (switches to AutoNoVsync), then 30 s of per-frame times from the
bench's `BenchFrames` capture plus `get_diagnostics` every 0.5 s: mean FPS, 1 % low, min, p50/p99/max.
Session B (`--features dev,profile`, TRACE_CHROME): the same scene, a 10 s window, clean shutdown, and
`tools/qa/trace.py` over it: top systems per frame, FixedMain per tick, AI and physics per tick against
the §11 budgets (physics + AI <= 4 ms per tick, AI alone <= 1.5 ms). `RUST_LOG` keeps only the spans
the summary reads (systems, schedules, frames) and errors: unfiltered, the trace grew ~240 MB/s (11 GB
per run, the shutdown flush > 15 s).
Bench cheats (`src/bench/`, active only under `--bench-scene`): 5 stars pinned with the player in
sight, the player and the driven car at full health, and the car a ghost on rails: it follows its lane
route kinematically at `bench.speed` (render.ron) and collides with nothing, so traffic and the police
cannot box it in while the chase and traffic run as usual around it.
Precondition of the verdict (GATE BROKEN otherwise, no verdict): in each measured window the car is
faster than `bench.moving_speed` (render.ron) in at least 90 % of the speed samples.
Hard failures: the composition is not reached, a crash, ERROR lines in the log, a missing or unparsable
trace, a standing bench car."""

import argparse
import json
import math
from pathlib import Path
import re
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from brp import Game, REPO, load_golden, vec3  # noqa: E402
import trace as chrome_trace  # noqa: E402
from t5 import chunk_count, game_state, resource_value, screenshot, wait_chunks  # noqa: E402
from t6 import rows  # noqa: E402
from t8 import scalar, variant  # noqa: E402
from t11 import cops, active, wanted  # noqa: E402
from t15 import police_cars, civilians  # noqa: E402

SEED = 1
COMPOSE_S = 120.0
MEASURE_S = 30.0
TRACE_S = 10.0
SAMPLE_S = 0.5
MIN_TRAFFIC = 20
NEAR_CENTRE_M = 150.0
# GDD §11: 60 FPS at 1080p; physics + AI <= 4 ms per fixed tick, AI alone <= 1.5 ms.
BUDGET_MS = {"frame": 1000.0 / 60.0, "fixed_main": 4.0, "physics_plus_ai": 4.0, "ai": 1.5}
# Spans `trace.py` reads (systems, schedules, frames) plus every error line for the log check.
TRACE_FILTER = ("error,bevy_ecs::system::function_system=info,bevy_ecs::schedule::schedule=info,"
                "bevy_app::sub_app=info")
TRACE_FLUSH_S = 180
MOVING_SHARE = 0.9
BENCH_CHEATS = [
    "5 stars pinned, the player always in sight (WantedLevel)",
    "player health and armour held at max",
    "driven car health held at max",
    "car on rails: kinematic lane route at bench.speed, rightmost turn at every junction, "
    "collides with nothing (traffic and police cannot box it in)",
]
KEEP_TRACE = "--keep-trace" in sys.argv
# Police cars that no longer take part in the chase.
IDLE_CAR_STATES = ("Leave", "Taken", "Abandoned")


def render_ron():
    return (REPO / "assets" / "world" / "render.ron").read_text(encoding="utf-8")


def bench_data():
    """`bench.speed` / `bench.moving_speed` and the render chunk size from render.ron."""
    text = render_ron()
    bench = re.search(r"bench:\s*\((.*?)\),?\s*$", text, re.M)
    if bench is None:
        raise AssertionError("GATE BROKEN: no bench: (...) in render.ron")

    def number(source, name):
        found = re.search(rf"\b{name}:\s*([\d.]+)", source)
        if found is None:
            raise AssertionError(f"GATE BROKEN: no {name} in render.ron")
        return float(found.group(1))

    return {"speed": number(bench.group(1), "speed"), "moving_speed": number(bench.group(1), "moving_speed"),
            "chunk_size": number(text, "chunk_size")}


def row5():
    text = (REPO / "assets" / "police" / "escalation.ron").read_text(encoding="utf-8")
    found = re.findall(r"\(units:\s*(\d+),\s*swat:\s*\d+,.*?cars:\s*(\d+)\)", text)
    if len(found) != 5:
        raise AssertionError(f"GATE BROKEN: expected 5 star rows in escalation.ron, got {len(found)}")
    units, cars = found[-1]
    return {"units": int(units), "cars": int(cars)}


def driven_car(game):
    found = rows(game, ["Driving"], with_="Player")
    return found[0][1][0]["vehicle"] if found else None


def car_speed(game, car):
    for e, (v,) in rows(game, ["LinearVelocity"], with_="Vehicle"):
        if e == car:
            return math.sqrt(sum(c * c for c in vec3(v)))
    return None


def car_position(game, car):
    for e, (p,) in rows(game, ["Position"], with_="Vehicle"):
        if e == car:
            return vec3(p)
    return None


def traffic_counts(game):
    stats = resource_value(game, "TrafficStats")
    return {k: int(stats[k]) for k in ("cars", "spawned", "despawned")}


def composition(game, caps):
    me = rows(game, ["Position"], with_="Player")[0][1][0]
    centre = vec3(me)
    gang = [vec3(p) for _, (m, p) in rows(game, ["GangMember", "Position"]) if variant(m["state"]) != "Dead"]
    cars = [c for c in police_cars(game) if c["state"] not in IDLE_CAR_STATES]
    units = active(cops(game))
    # Crews aboard are not entities (PoliceCar.crew); the dispatcher counts them as units.
    aboard = [variant(k) for _, (c,) in rows(game, ["PoliceCar"]) if variant(c["state"]) not in IDLE_CAR_STATES
              for k in c["crew"]]
    kinds = [u["kind"] for u in units] + aboard
    return {
        "driving": driven_car(game) is not None,
        "stars": wanted(game)["stars"],
        "police_cars": len(cars),
        "police_units": len(units) + len(aboard),
        "units_on_foot": len(units),
        "units_aboard": len(aboard),
        "units_by_kind": {k: kinds.count(k) for k in sorted(set(kinds))},
        "traffic_cars": int(resource_value(game, "TrafficStats")["cars"]),
        "civilians": len([c for c in civilians(game) if c["state"] != "Dead"]),
        "gang_near_centre": sum(1 for p in gang if math.hypot(p[0] - centre[0], p[2] - centre[2]) <= NEAR_CENTRE_M),
        "caps": caps,
    }


def missing(c, traffic_max):
    """What the composition still lacks. Traffic counts its peak over the wait: the bubble count
    fluctuates by a car or two while the chase moves (19-23 in one run)."""
    caps = c["caps"]
    parts = []
    if not c["driving"]:
        parts.append("the player drives")
    if c["stars"] != 5:
        parts.append(f"5 stars (got {c['stars']})")
    if c["police_cars"] != caps["cars"]:
        parts.append(f"{caps['cars']} police cars (got {c['police_cars']})")
    if c["police_units"] != caps["units"]:
        parts.append(f"{caps['units']} police units (got {c['police_units']})")
    if traffic_max < MIN_TRAFFIC:
        parts.append(f">= {MIN_TRAFFIC} traffic cars (peak {traffic_max})")
    return parts


def boot(game):
    deadline = time.monotonic() + 180
    while True:
        try:
            if game_state(game) == "Playing":
                break
        except RuntimeError:
            pass
        if time.monotonic() > deadline:
            raise AssertionError("GameState never became Playing")
        time.sleep(0.1)
    layout_hash = int(scalar(resource_value(game, "CityLayoutHash")))
    if layout_hash != load_golden()[SEED]:
        raise AssertionError(f"layout hash {layout_hash:#x} is not the golden hash of seed {SEED}")
    return wait_chunks(game)


def compose(game, caps):
    """Waits for the bench composition; returns it and the car speed samples (1 s) meanwhile."""
    deadline = time.monotonic() + COMPOSE_S
    speeds = []
    last = None
    traffic_max = 0
    while time.monotonic() < deadline:
        last = composition(game, caps)
        traffic_max = max(traffic_max, last["traffic_cars"])
        last["traffic_peak"] = traffic_max
        car = driven_car(game)
        if car is not None:
            speeds.append(car_speed(game, car))
        if not missing(last, traffic_max):
            return last, speeds
        time.sleep(1.0)
    lacking = ", ".join(missing(last, traffic_max))
    raise AssertionError(f"GATE BROKEN: bench scene never reached {lacking} in {COMPOSE_S} s: {last}")


def window_size(game):
    window = game.component_path("Window")
    value = game.query([window], with_=[window])[0]["components"][window]
    res = value["resolution"]
    return {"physical_width": res.get("physical_width"), "physical_height": res.get("physical_height"),
            "scale_factor_override": res.get("scale_factor_override")}


def set_recording(game, on):
    game.call("world.mutate_resources", {
        "resource": game.resource_path("BenchFrames"), "path": ".recording", "value": on,
    })


def frame_metrics(frame_ms):
    if not frame_ms:
        raise AssertionError("GATE BROKEN: BenchFrames recorded no frame")
    ordered = sorted(frame_ms)
    n = len(ordered)
    slowest = ordered[-max(1, n // 100):]

    def pct(p):
        return ordered[min(n - 1, int(p / 100.0 * n))]

    return {
        "frames": n,
        "mean_fps": 1000.0 * n / sum(ordered),
        "low_1pct_fps": 1000.0 / (sum(slowest) / len(slowest)),
        "min_fps": 1000.0 / ordered[-1],
        "p50_ms": pct(50),
        "p99_ms": pct(99),
        "max_ms": ordered[-1],
    }


def sample_diagnostics(game, seconds, speeds=None, track=None):
    samples = []
    car = driven_car(game)
    t0 = time.monotonic()
    while time.monotonic() - t0 < seconds:
        time.sleep(SAMPLE_S)
        d = game.diagnostics()
        samples.append({"fps": d["fps"]["average"], "frame_ms": d["frame_time_ms"]["average"]})
        if speeds is not None and car is not None and len(samples) % 2 == 0:
            speeds.append(car_speed(game, car))
            if track is not None:
                track.append(car_position(game, car))
    return {
        "samples": len(samples),
        "fps_avg": sum(s["fps"] for s in samples) / len(samples),
        "fps_min": min(s["fps"] for s in samples),
        "frame_ms_avg": sum(s["frame_ms"] for s in samples) / len(samples),
        "frame_ms_max": max(s["frame_ms"] for s in samples),
    }


def moving_share(speeds, moving_speed):
    """Share of the samples with the car faster than `moving_speed`; a missing sample counts as standing."""
    return round(sum(1 for s in speeds if s is not None and s > moving_speed) / max(1, len(speeds)), 2)


def streaming(game, track, chunk_size, before, chunks_before):
    """What the drive streamed through in the window: render chunks resident before/after, distinct
    chunks the car passed, metres driven, traffic spawned/despawned."""
    points = [p for p in track if p is not None]
    after = traffic_counts(game)
    return {
        "chunks_resident": [chunks_before, chunk_count(game)],
        "chunks_visited": len({(math.floor(p[0] / chunk_size), math.floor(p[2] / chunk_size)) for p in points}),
        "metres_driven": round(sum(math.hypot(b[0] - a[0], b[2] - a[2]) for a, b in zip(points, points[1:])), 1),
        "traffic_cars": [before["cars"], after["cars"]],
        "traffic_spawned": after["spawned"] - before["spawned"],
        "traffic_despawned": after["despawned"] - before["despawned"],
    }


def require_moving(label, speeds, bench):
    share = moving_share(speeds, bench["moving_speed"])
    if share < MOVING_SHARE:
        raise AssertionError(
            f"GATE BROKEN: the bench car moved faster than {bench['moving_speed']} m/s in only {share:.0%} of "
            f"the {label} window (needs {MOVING_SHARE:.0%}); no verdict")
    return share


def log_errors(game):
    return [line for line in game.log_tail(4_000_000).splitlines() if "ERROR" in line]


def session_a(out, caps, bench, summary):
    with Game(features=("dev",), args=("--seed", str(SEED), "--bench-scene"), release=True) as game:
        summary["chunks"] = boot(game)
        comp, speeds = compose(game, caps)
        summary["composition"] = comp
        summary["window"] = window_size(game)
        shots = []
        for k in range(3):
            shots.append(screenshot(game, out / f"bench_{k}.png"))
            time.sleep(0.5)
        summary["screenshots"] = shots
        summary["frame_report"] = game.frame_report()
        traffic_before, chunks_before = traffic_counts(game), chunk_count(game)
        set_recording(game, True)
        measured, track = [], []
        summary["diagnostics"] = sample_diagnostics(game, MEASURE_S, measured, track)
        set_recording(game, False)
        summary["streaming"] = streaming(game, track, bench["chunk_size"], traffic_before, chunks_before)
        frame_ms = resource_value(game, "BenchFrames")["frame_ms"]
        summary["frames"] = frame_metrics([float(v) for v in frame_ms])
        windows = {"composition": speeds, "measured": measured}
        summary["car_speed_mps"] = {k: [None if s is None else round(s, 1) for s in v] for k, v in windows.items()}
        summary["car_moving_share"] = {k: moving_share(v, bench["moving_speed"]) for k, v in windows.items()}
        summary["composition_after"] = composition(game, caps)
        errors = log_errors(game)
        summary["log_errors_a"] = errors
        if errors:
            raise AssertionError("errors in the game log:\n" + "\n".join(errors[:20]))
        game.shutdown()
        game.process.wait(timeout=15)
    require_moving("measured", measured, bench)


def session_b(out, caps, bench, summary):
    path = out / "trace.json"
    path.unlink(missing_ok=True)
    env = {"TRACE_CHROME": str(path), "RUST_LOG": TRACE_FILTER}
    with Game(features=("dev", "profile"), args=("--seed", str(SEED), "--bench-scene"), release=True,
              env=env) as game:
        boot(game)
        comp, _ = compose(game, caps)
        summary["trace_composition"] = comp
        traced = []
        summary["trace_diagnostics"] = {"label": "with tracing", **sample_diagnostics(game, TRACE_S, traced)}
        summary["trace_car_speed_mps"] = [None if s is None else round(s, 1) for s in traced]
        summary["trace_car_moving_share"] = moving_share(traced, bench["moving_speed"])
        errors = log_errors(game)
        summary["log_errors_b"] = errors
        if errors:
            raise AssertionError("errors in the game log (trace session):\n" + "\n".join(errors[:20]))
        game.shutdown()
        game.process.wait(timeout=TRACE_FLUSH_S)
    require_moving("traced", traced, bench)
    if not path.is_file():
        raise AssertionError(f"trace {path} missing")
    size = path.stat().st_size
    with open(path, "rb") as f:
        f.seek(max(0, size - 16))
        clean = f.read().rstrip().endswith(b"]")
    summary["trace_file"] = {"path": str(path), "bytes": size, "clean_end": clean}
    if not clean:
        print("trace truncated: parsing what was written")
    try:
        t = chrome_trace.summarize(path, TRACE_S)
    except Exception as error:
        raise AssertionError(f"trace unparsable: {error}") from error
    if not t["frames"]["count"] or not t["fixed_main"]["count"]:
        raise AssertionError(f"trace has no frame or FixedMain span: {t['frames']} {t['fixed_main']}")
    if not KEEP_TRACE:
        # 10-16 GB per run: the summary keeps what the verdict needs.
        path.unlink()
        summary["trace_file"]["deleted_after_parse"] = True
    summary["trace"] = {
        "frames": t["frames"],
        "fixed_main": t["fixed_main"],
        "top15_ms_per_frame": [(n, round(ms, 4)) for n, ms in t["systems_ms_per_frame"][:15]],
        "ms_per_tick": t["ms_per_tick"],
        "unclassified_modules": t["unclassified_modules"],
    }


def verdict(summary):
    frames, trace = summary["frames"], summary["trace"]
    over = []
    if frames["p99_ms"] > BUDGET_MS["frame"]:
        over.append(f"frame p99 {frames['p99_ms']:.2f} ms > {BUDGET_MS['frame']:.1f} ms")
    if trace["fixed_main"]["mean_ms"] > BUDGET_MS["fixed_main"]:
        over.append(f"FixedMain {trace['fixed_main']['mean_ms']:.2f} ms/tick > {BUDGET_MS['fixed_main']} ms")
    if trace["ms_per_tick"]["ai"] > BUDGET_MS["ai"]:
        over.append(f"AI {trace['ms_per_tick']['ai']:.2f} ms/tick > {BUDGET_MS['ai']} ms")
    both = trace["ms_per_tick"]["physics"] + trace["ms_per_tick"]["ai"]
    if both > BUDGET_MS["physics_plus_ai"]:
        over.append(f"physics + AI {both:.2f} ms/tick > {BUDGET_MS['physics_plus_ai']} ms")
    end = summary["composition_after"]
    scene = (f"at the measured scene ({end['traffic_cars']} traffic cars, {end['gang_near_centre']} gang members "
             f"near the centre, {end['stars']} stars, {end['police_cars']} police cars, {end['police_units']} units, "
             f"bench car on rails moving {summary['car_moving_share']['measured']:.0%} of the measured window, "
             f"{summary['streaming']['metres_driven']} m driven)")
    text = "; ".join(over) if over else "nothing to fix by trace: every budget holds"
    return {"budgets_ms": BUDGET_MS, "over_budget": over, "text": f"{scene}: {text}"}


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    check = subprocess.run([sys.executable, str(REPO / "tools" / "fetch_assets.py"), "--check"], cwd=REPO)
    if check.returncode != 0:
        raise AssertionError("run python tools/fetch_assets.py first")
    caps = row5()
    bench = bench_data()
    summary = {"seed": SEED, "caps": caps, "bench": bench, "bench_cheats": BENCH_CHEATS}
    try:
        session_a(out, caps, bench, summary)
        session_b(out, caps, bench, summary)
        summary["verdict"] = verdict(summary)
    finally:
        (out / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(summary, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t16")
    parser.add_argument("--keep-trace", action="store_true", help="keep trace.json (10-16 GB)")
    run(parser.parse_args().out.resolve())
