"""Runtime T15 gate: traffic and police cars in the seed-1 city.

1. Traffic: at least 12 `TrafficCar`s appear; their speeds (mean over the cars > 3 m/s, none above
   16.5 m/s); a street screenshot.
2. Hijack: the nearest kinematic traffic car; the player stands in its lane 12 m ahead (it stops), then
   in front of its bumper on the driver's side, inside its own lane (the car keeps standing for him and
   oncoming traffic passes clear; 1.5 m out of the left door was the oncoming lane, QA round 2), within
   `enter_radius` of the door point. F once the player's `HitReaction` is `Steady` and the car still
   stands: `Driving` is that car within 1 s and a fleeing `Civilian` (the thrown-out driver) stands
   within 3 m.
3. Chase: `WantedLevel.heat` = 180 (2 stars, re-raised if it lapses); W in bursts along the street;
   every 0.5 s the flat distance of each active `PoliceCar` and each live `PoliceUnit` to the player:
   never more than 2 active police cars (asserted). Police pressure is a REPORTED metric, not a gate
   (TASK-016 final decision: police cars in traffic may lose a fleeing car): distance series, which kind
   first came within `PRESSURE_M`, time to it, or an escape (hidden timer / distance opening past the
   spawn ring). A screenshot every second.
4. Stop and F out: whether a `Dismounted` police car and its crew show up within 20 s (reported).
5. `get_diagnostics` and `Game.frame_report()` (FPS only from there).
6. Shutdown; no ERROR_WORDS in the log.

Enum names are read from the Rust source, never hard-coded lists (TASK-015). BRP constraint: every key
is held once per window, an entity is re-queried right before it is mutated. Hard pass/fail rests on
components; screenshots are evidence for the owner (density, turns, chase feel, sirens, models)."""

import argparse
import json
import math
from pathlib import Path
import re
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from brp import Game, REPO, vec3  # noqa: E402
from t5 import ERROR_WORDS, game_state, poll, screenshot, wait_chunks  # noqa: E402
from t6 import rows  # noqa: E402
from t8 import scalar, variant  # noqa: E402
from t11 import set_heat, wanted  # noqa: E402
from t5 import resource_value  # noqa: E402
from t14 import driving, put, quat, ron_number, ron_tuple, rotate  # noqa: E402
from t7 import reaction_name  # noqa: E402

SEED = 1
MIN_TRAFFIC = 12
MEAN_SPEED = 3.0
MAX_SPEED = 16.5
AHEAD = 12.0
STOPPED = 0.5
# The stand point in front of the bumper: this far in from the door point's side, and this much air
# between the capsule and the chassis box, m.
STAND_IN = 0.5
STAND_AIR = 0.25
FLEE_NEAR = 3.0
CHASE_S = 25.0
PRESSURE_M = 18.0
SAMPLE_S = 0.5
BURST_MS = 1500
DISMOUNT_S = 20.0
CREW_NEAR = 40.0


def enum_variants(rel, name):
    """Variant names of `enum name` in a Rust source file."""
    text = (REPO / rel).read_text(encoding="utf-8")
    match = re.search(rf"pub enum {name}\s*\{{(.*?)\n\}}", text, re.S)
    if not match:
        raise AssertionError(f"GATE BROKEN: enum {name} not found in {rel}")
    body = re.sub(r"\{[^{}]*\}", "", match.group(1))
    body = re.sub(r"//[^\n]*", "", body)
    return [v.strip() for v in body.split(",") if v.strip()]


def wanted_raw(game):
    try:
        return resource_value(game, "WantedLevel")
    except Exception:
        return None


def flat(a, b):
    return math.hypot(a[0] - b[0], a[2] - b[2])


def speed(v):
    return math.sqrt(sum(c * c for c in v))


def me(game):
    found = rows(game, ["Position"], with_="Player")
    if len(found) != 1:
        raise AssertionError(f"expected one player, found {len(found)}")
    return {"entity": found[0][0], "position": vec3(found[0][1][0])}


def steady(game):
    found = rows(game, ["HitReaction"], with_="Player")
    return len(found) == 1 and reaction_name(found[0][1][0]) == "Steady"


def traffic(game):
    return [
        {"entity": e, "mode": variant(t["mode"]), "position": vec3(p), "velocity": vec3(v),
         "rotation": quat(r)}
        for e, (t, p, v, r) in rows(game, ["TrafficCar", "Position", "LinearVelocity", "Rotation"])
    ]


def police_cars(game):
    return [
        {"entity": e, "state": variant(c["state"]), "position": vec3(p)}
        for e, (c, p) in rows(game, ["PoliceCar", "Position"])
    ]


def civilians(game):
    return [
        {"entity": e, "state": variant(c["state"]), "position": vec3(p)}
        for e, (c, p) in rows(game, ["Civilian", "Position"])
    ]


def police_units(game):
    return [{"entity": e, "state": variant(u["state"]), "position": vec3(p)}
            for e, (u, p) in rows(game, ["PoliceUnit", "Position"])]


def pressure_seconds():
    """Data bound for police pressure at 2 stars: a car from the outer ring edge at pursuit speed, held up
    for the in-box limit (blocked x junction factor), the driver's car standing `stopped_seconds` for a
    dismount, then the crew running the rest of the ring to `PRESSURE_M`."""
    text = (REPO / "assets" / "police" / "escalation.ron").read_text(encoding="utf-8")
    car = re.search(r"\bcar:\s*\((.*?)\),\s*\n", text, re.S)
    if not car:
        raise AssertionError("GATE BROKEN: car block not found in police/escalation.ron")
    block = car.group(1)

    def field(name):
        match = re.search(rf"\b{name}:\s*([-\d.]+)", block)
        if not match:
            raise AssertionError(f"GATE BROKEN: car.{name} not found in police/escalation.ron")
        return float(match.group(1))
    ring = re.search(r"\bspawn_ring:\s*\(\s*([\d.]+)\s*,\s*([\d.]+)\s*\)", block)
    if not ring:
        raise AssertionError("GATE BROKEN: car.spawn_ring not found in police/escalation.ron")
    outer = float(ring.group(2))
    run_speed = ron_number("character/locomotion.ron", "run_speed")
    return (outer / field("pursuit_speed") + field("blocked_seconds") * field("junction_factor")
            + field("stopped_seconds") + (outer - PRESSURE_M) / run_speed)


def ring_outer():
    """The farthest spawn ring edge (foot or car) in police/escalation.ron, m."""
    text = (REPO / "assets" / "police" / "escalation.ron").read_text(encoding="utf-8")
    rings = re.findall(r"spawn_ring:\s*\(\s*[\d.]+\s*,\s*([\d.]+)\s*\)", text)
    if not rings:
        raise AssertionError("GATE BROKEN: spawn_ring not found in police/escalation.ron")
    return max(float(r) for r in rings)


def crew(game):
    return [{"entity": e, "car": scalar(c), "position": vec3(p)}
            for e, (c, p) in rows(game, ["CrewOf", "Position"])]


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    check = subprocess.run([sys.executable, str(REPO / "tools" / "fetch_assets.py"), "--check"], cwd=REPO)
    if check.returncode != 0:
        raise AssertionError("run python tools/fetch_assets.py first")
    modes = enum_variants("crates/gta_sim/src/traffic/mod.rs", "TrafficMode")
    car_states = enum_variants("crates/gta_sim/src/police/cars.rs", "PoliceCarState")
    civilian_states = enum_variants("crates/gta_sim/src/civilian/mod.rs", "CivilianState")
    for needed, names in (("Kinematic", modes), ("Taken", modes), ("Dismounted", car_states),
                          ("Flee", civilian_states)):
        if needed not in names:
            raise AssertionError(f"GATE BROKEN: {needed} not in {names}")
    active_states = [s for s in car_states if s in ("Respond", "Chase", "Dismounted")]
    door = ron_tuple("vehicle/sedan.ron", "door")
    half_z = ron_tuple("vehicle/sedan.ron", "chassis_half_extents")[2]
    radius = ron_number("character/locomotion.ron", "capsule_radius")
    enter_radius = ron_number("vehicle/sedan.ron", "enter_radius")
    stand_local = (door[0] + STAND_IN, 0.0, -(half_z + radius + STAND_AIR))
    if math.hypot(stand_local[0] - door[0], stand_local[2] - door[2]) >= enter_radius - 0.1:
        raise AssertionError(f"GATE BROKEN: stand point {stand_local} out of reach of the door {door}")
    float_height = ron_number("character/locomotion.ron", "float_height")
    summary = {"seed": SEED, "enums": {"TrafficMode": modes, "PoliceCarState": car_states}}
    try:
        with Game(features=("dev",), args=("--seed", str(SEED)), release=True) as game:
            paths = (game.component_path("Position"), game.component_path("Transform"),
                     game.component_path("Rotation"))
            poll("GameState Playing", lambda: game_state(game) == "Playing", 180, 0.05)
            summary["chunks"] = wait_chunks(game)

            # 1. Traffic density and speeds.
            poll(f">= {MIN_TRAFFIC} traffic cars", lambda: len(traffic(game)) >= MIN_TRAFFIC, 30, 0.25)
            speeds = {}
            for _ in range(8):
                for car in traffic(game):
                    speeds.setdefault(car["entity"], []).append(speed(car["velocity"]))
                time.sleep(0.25)
            means = [sum(v) / len(v) for v in speeds.values()]
            top = max(max(v) for v in speeds.values())
            summary["traffic"] = {"cars": len(speeds), "mean_speed": sum(means) / len(means), "max_speed": top,
                                  "street_png": screenshot(game, out / "street.png")}
            if summary["traffic"]["mean_speed"] <= MEAN_SPEED:
                raise AssertionError(f"traffic crawls: {summary['traffic']}")
            if top > MAX_SPEED:
                raise AssertionError(f"a traffic car at {top:.1f} m/s")

            # 2. Hijack the nearest kinematic car.
            here = me(game)
            kinematic = [c for c in traffic(game) if c["mode"] == "Kinematic"]
            target = min(kinematic, key=lambda c: flat(c["position"], here["position"]))
            forward = rotate(target["rotation"], (0.0, 0.0, -1.0))
            block = (target["position"][0] + forward[0] * AHEAD, float_height,
                     target["position"][2] + forward[2] * AHEAD)
            put(game, here["entity"], paths, block)
            poll("the car stops for the player", lambda: next(
                (speed(c["velocity"]) <= STOPPED for c in traffic(game) if c["entity"] == target["entity"]),
                False), 10, 0.1)
            poll("the player back on his feet", lambda: steady(game), 10, 0.05)
            car = next(c for c in traffic(game) if c["entity"] == target["entity"])
            offset = rotate(car["rotation"], stand_local)
            stand = (car["position"][0] + offset[0], float_height, car["position"][2] + offset[2])
            put(game, me(game)["entity"], paths, stand)
            time.sleep(0.2)
            poll("the player steady at the door", lambda: steady(game), 10, 0.05)

            def target_speed():
                return next((speed(c["velocity"]) for c in traffic(game) if c["entity"] == target["entity"]),
                            None)
            before_f = target_speed()
            if before_f is None or before_f > STOPPED:
                raise AssertionError(f"GATE BROKEN: the target car is not standing before F: {before_f}")
            game.send_keys(["KeyF"], 100)
            poll("Driving the traffic car", lambda: driving(game) == target["entity"], 1.0, 0.05)
            fleeing = [c for c in civilians(game) if c["state"] == "Flee" and flat(c["position"], stand) <= FLEE_NEAR]
            summary["hijack"] = {"car": target["entity"], "fleeing_near": len(fleeing), "speed_before_f": before_f,
                                 "png": screenshot(game, out / "hijack.png")}
            if not fleeing:
                raise AssertionError(f"no fleeing driver within {FLEE_NEAR} m: {civilians(game)[:5]}")

            # 3. Two stars and a chase.
            within = pressure_seconds()
            if within > CHASE_S:
                raise AssertionError(f"GATE BROKEN: pressure bound {within:.1f} s exceeds the {CHASE_S} s chase")
            set_heat(game, 180)
            poll("2 stars", lambda: wanted(game)["stars"] == 2, 3, 0.05)
            samples, most_active, shots, pressure, hidden_at = [], 0, [], None, None
            start = time.monotonic()
            next_burst = start
            next_shot = start
            while time.monotonic() - start < CHASE_S:
                now = time.monotonic()
                if now >= next_burst:
                    game.send_keys(["KeyW"], BURST_MS)
                    next_burst = now + 2.0 * BURST_MS / 1000.0
                if now >= next_shot:
                    shots.append(screenshot(game, out / f"chase_{len(shots)}.png"))
                    next_shot = now + 1.0
                # The hidden timer is read before any re-raise: it is the escape signal.
                level = wanted_raw(game) or {}
                if hidden_at is None and float(level.get("hidden") or 0.0) > 0.0:
                    hidden_at = round(now - start, 2)
                # Named mutation: the wanted level must not lapse during the chase (driving off unseen for
                # 15 s outside the 70 m circle clears 2 stars); re-raised heat re-centres the search on
                # the player.
                if int(level.get("stars") or 0) < 2:
                    set_heat(game, 180)
                player_at = me(game)["position"]
                cars = police_cars(game)
                active = [c for c in cars if c["state"] in active_states]
                most_active = max(most_active, len(active))
                car_d = min((flat(c["position"], player_at) for c in active), default=None)
                foot_d = min((flat(u["position"], player_at) for u in police_units(game)
                              if u["state"] != "Dead"), default=None)
                samples.append({"t": round(now - start, 2), "car": car_d, "foot": foot_d})
                if pressure is None:
                    near = [(d, kind) for kind, d in (("car", car_d), ("foot", foot_d))
                            if d is not None and d <= PRESSURE_M]
                    if near:
                        d, kind = min(near)
                        pressure = {"t": round(now - start, 2), "kind": kind, "distance": round(d, 1)}
                time.sleep(SAMPLE_S)
            # Reported metric, not a gate (TASK-016 final decision): police cars in traffic may lose a
            # fleeing car. Escape = the hidden timer started, or the nearest unit kept opening past the ring.
            outer = ring_outer()
            tail = [min(d for d in (s["car"], s["foot"]) if d is not None) for s in samples[-6:]
                    if s["car"] is not None or s["foot"] is not None]
            opening = (len(tail) >= 2 and tail[-1] > outer
                       and all(b >= a for a, b in zip(tail, tail[1:])))
            escaped = pressure is None and (hidden_at is not None or opening or not tail)
            summary["chase"] = {"most_active": most_active, "pressure": pressure,
                                "first_within_m": PRESSURE_M,
                                "first_kind": pressure["kind"] if pressure else None,
                                "time_to_pressure_s": pressure["t"] if pressure else None,
                                "escaped": escaped, "hidden_started_s": hidden_at,
                                "distance_opening_past_ring": opening,
                                "pressure_bound_s": round(within, 2),
                                "car_series": [s["car"] for s in samples],
                                "foot_series": [s["foot"] for s in samples],
                                "samples": samples, "screenshots": shots}
            if pressure:
                print(f"CHASE: pressure at {pressure['t']} s by {pressure['kind']}")
            elif escaped:
                print("CHASE: player escaped")
            else:
                print("CHASE: no pressure, no escape signal")
            if most_active > 2:
                raise AssertionError(f"{most_active} active police cars at 2 stars")

            # 4. Stop, get out, the crew gets out too.
            time.sleep(2.0 * BURST_MS / 1000.0)
            poll("stopped", lambda: all(speed(c["velocity"]) < 3.0 for c in traffic(game)
                                         if c["entity"] == target["entity"]), 10, 0.2)
            game.send_keys(["KeyF"], 100)
            poll("on foot", lambda: driving(game) is None, 3, 0.05)

            trace = []

            def dismounted():
                if wanted(game)["stars"] < 2:
                    set_heat(game, 180)
                here_now = me(game)["position"]
                cars = police_cars(game)
                trace.append([(c["state"], round(flat(c["position"], here_now), 1)) for c in cars]
                             + [("last_known", (wanted_raw(game) or {}).get("last_known"))])
                dismounted_cars = [c for c in cars if c["state"] == "Dismounted"]
                near = [c for c in crew(game) if flat(c["position"], here_now) <= CREW_NEAR]
                return dismounted_cars and near
            # Reported, not asserted: after an escape no police car is near to dismount.
            try:
                poll("a dismounted police car and its crew", dismounted, DISMOUNT_S, 0.25)
                dismount_ok = True
            except AssertionError:
                dismount_ok = False
            summary["dismount_trace"] = trace[::4]
            summary["dismount"] = {"reached": dismount_ok, "cars": [c["state"] for c in police_cars(game)], "crew": len(crew(game)),
                                   "png": screenshot(game, out / "dismount.png")}

            # 5. Diagnostics and frame report.
            summary["diagnostics"] = game.diagnostics()
            summary["frame_report"] = game.frame_report()
            lines = game.log_tail(2_000_000).splitlines()
            errors = [line for line in lines
                      if "ERROR" in line and any(w.lower() in line.lower() for w in ERROR_WORDS)]
            summary["log_errors"] = errors
            if errors:
                raise AssertionError(f"errors in game.log: {errors[:5]}")
        summary["result"] = "PASS"
    except Exception as err:
        summary["result"] = "FAIL"
        summary["error"] = str(err)
        raise
    finally:
        (out / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False, default=str),
                                          encoding="utf-8")
        print(json.dumps(summary, indent=2, ensure_ascii=False, default=str))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t15")
    run(parser.parse_args().out)


if __name__ == "__main__":
    main()
