"""Runtime T14 gate: the car. In the seed-1 city the player is put at the driver's door of the nearest
parked car and presses F (`Driving` appears), holds W for 3 s (the car moves and gathers speed, the
engine hum is spawned, the other cars are dots on the minimap), is then set 30 m in front of the city
edge wall facing it and holds W for 4 s (the crash costs car health, the car stops at the wall face),
and presses F again (on foot next to the car, still `Playing`).

BRP constraint: every key is held once per window (bevy_brp_extras releases a hold on its own timer),
and an entity is re-queried right before it is mutated.

Hard pass/fail rests on components (`Driving`, `Vehicle`, `Position`, `LinearVelocity`, `VehicleHealth`,
`SoundStats`, `MinimapMarker`, `GameState`); screenshots are evidence for the owner, who judges the
handling, camera, sound and visuals (owner checklist in the task summary)."""

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
from t5 import ERROR_WORDS, game_state, poll, resource_value, screenshot, wait_chunks  # noqa: E402
from t6 import rows  # noqa: E402
from t8 import scalar, variant  # noqa: E402

SEED = 1
DRIVE_MS = 3000
SHOT_EVERY_S = 0.5
CRASH_MS = 4000
# Index of `SoundClass::Engine` (declaration order in src/audio/cues.rs).
ENGINE_CLASS = 8
MIN_SPEED = 5.0
MIN_TRAVEL = 5.0
WALL_FACE_Z = 700.0
WALL_START_Z = 670.0
STOPPED = 2.0
EXIT_REACH = 3.0


def ron_tuple(rel, name):
    text = (REPO / "assets" / rel).read_text(encoding="utf-8")
    match = re.search(rf"\b{name}:\s*\(\s*([-\d.]+)\s*,\s*([-\d.]+)\s*,\s*([-\d.]+)\s*\)", text)
    if not match:
        raise AssertionError(f"GATE BROKEN: {name} not found in {rel}")
    return tuple(float(v) for v in match.groups())


def ron_number(rel, name):
    text = (REPO / "assets" / rel).read_text(encoding="utf-8")
    match = re.search(rf"\b{name}:\s*([-\d.]+)", text)
    if not match:
        raise AssertionError(f"GATE BROKEN: {name} not found in {rel}")
    return float(match.group(1))


def quat(raw):
    """avian `Rotation` (a newtype over Quat) as (x, y, z, w)."""
    while isinstance(raw, list) and len(raw) == 1:
        raw = raw[0]
    if isinstance(raw, dict):
        return raw["x"], raw["y"], raw["z"], raw["w"]
    return tuple(raw)


def rotate(q, v):
    x, y, z, w = q
    u = (x, y, z)
    uv = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
    uuv = (u[1] * uv[2] - u[2] * uv[1], u[2] * uv[0] - u[0] * uv[2], u[0] * uv[1] - u[1] * uv[0])
    return tuple(v[i] + 2.0 * (w * uv[i] + uuv[i]) for i in range(3))


def flat(a, b):
    return math.hypot(a[0] - b[0], a[2] - b[2])


def cars(game):
    return [
        {"entity": e, "position": vec3(p), "rotation": quat(r)}
        for e, (_, p, r) in rows(game, ["Vehicle", "Position", "Rotation"])
    ]


def car(game, entity):
    for e, (p, v, h) in rows(game, ["Position", "LinearVelocity", "VehicleHealth"], with_="Vehicle"):
        if e == entity:
            return {"position": vec3(p), "velocity": vec3(v), "health": float(scalar(h))}
    raise AssertionError(f"car {entity} is gone")


def player(game):
    found = rows(game, ["Position", "Health"], with_="Player")
    if len(found) != 1:
        raise AssertionError(f"expected one player, found {len(found)}")
    entity, (position, health) = found[0]
    return {"entity": entity, "position": vec3(position), "health": health}


def driving(game):
    found = rows(game, ["Driving"], with_="Player")
    return found[0][1][0]["vehicle"] if found else None


def speed(velocity):
    return math.sqrt(sum(c * c for c in velocity))


def put(game, entity, component_paths, position, rotation=None):
    """Position (+ Transform) and optionally Rotation of an entity, re-queried first."""
    pos_path, transform_path, rot_path = component_paths
    if not any(row["entity"] == entity for row in game.query([pos_path], with_=[pos_path])):
        raise AssertionError(f"entity {entity} vanished before the teleport")
    game.mutate_component(entity, pos_path, "", list(position))
    game.mutate_component(entity, transform_path, ".translation", list(position))
    if rotation is not None:
        game.mutate_component(entity, rot_path, "", list(rotation))
        game.mutate_component(entity, transform_path, ".rotation", list(rotation))


def engine_spawned(game):
    return int(resource_value(game, "SoundStats")["spawned"][ENGINE_CLASS])


def vehicle_markers(game):
    return sum(1 for _, (m,) in rows(game, ["MinimapMarker"]) if variant(m["kind"]) == "Vehicle")


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    check = subprocess.run([sys.executable, str(REPO / "tools" / "fetch_assets.py"), "--check"], cwd=REPO)
    if check.returncode != 0:
        raise AssertionError("run python tools/fetch_assets.py first")
    door = ron_tuple("vehicle/sedan.ron", "door")
    half_z = ron_tuple("vehicle/sedan.ron", "chassis_half_extents")[2]
    float_height = ron_number("character/locomotion.ron", "float_height")
    summary = {"seed": SEED}
    try:
        with Game(features=("dev",), args=("--seed", str(SEED)), release=True) as game:
            paths = (game.component_path("Position"), game.component_path("Transform"),
                     game.component_path("Rotation"))
            vel_path = game.component_path("LinearVelocity")
            ang_path = game.component_path("AngularVelocity")
            poll("GameState Playing", lambda: game_state(game) == "Playing", 180, 0.05)
            summary["chunks"] = wait_chunks(game)
            parked = cars(game)
            summary["parked_cars"] = len(parked)
            if not parked:
                raise AssertionError("no parked cars in the city")

            # 1. At the driver's door of the nearest car, F.
            me = player(game)
            target = min(parked, key=lambda c: flat(c["position"], me["position"]))
            offset = rotate(target["rotation"], door)
            at = (target["position"][0] + offset[0], float_height, target["position"][2] + offset[2])
            put(game, me["entity"], paths, at)
            time.sleep(0.3)
            game.send_keys(["KeyF"], 100)
            poll("Driving", lambda: driving(game) == target["entity"], 2, 0.05)
            summary["entered"] = target["entity"]
            time.sleep(0.3)

            # 2. W for 3 s: the car moves, the engine hums, other cars are on the minimap.
            start = car(game, target["entity"])["position"]
            game.send_keys(["KeyW"], DRIVE_MS)
            shots, top = [], 0.0
            deadline = time.monotonic() + DRIVE_MS / 1000.0 - 0.2
            k = 0
            while time.monotonic() < deadline:
                top = max(top, speed(car(game, target["entity"])["velocity"]))
                shots.append(screenshot(game, out / f"drive_{k}.png"))
                k += 1
                time.sleep(max(SHOT_EVERY_S - 0.15, 0.15))
            now = car(game, target["entity"])
            top = max(top, speed(now["velocity"]))
            travel = flat(now["position"], start)
            summary["drive"] = {"top_speed": top, "travel": travel, "screenshots": shots}
            if top < MIN_SPEED:
                raise AssertionError(f"top speed {top:.2f} m/s < {MIN_SPEED}")
            if travel < MIN_TRAVEL:
                raise AssertionError(f"the car moved {travel:.2f} m < {MIN_TRAVEL}")
            summary["engine_spawned"] = engine_spawned(game)
            if summary["engine_spawned"] < 1:
                raise AssertionError("no engine sound was spawned")
            summary["vehicle_markers"] = vehicle_markers(game)
            if summary["vehicle_markers"] < 1:
                raise AssertionError("no car dots on the minimap")
            time.sleep(1.0)

            # 3. Into the city edge wall from 30 m.
            facing_z = (0.0, 1.0, 0.0, 0.0)  # yaw 180: forward +Z
            rest_y = start[1]
            put(game, target["entity"], paths, (0.0, rest_y, WALL_START_Z), facing_z)
            game.mutate_component(target["entity"], vel_path, "", [0.0, 0.0, 0.0])
            game.mutate_component(target["entity"], ang_path, "", [0.0, 0.0, 0.0])
            time.sleep(2.0)
            before = car(game, target["entity"])
            summary["before_crash"] = before
            game.send_keys(["KeyW"], CRASH_MS)
            time.sleep(CRASH_MS / 1000.0 + 1.0)
            after = car(game, target["entity"])
            summary["after_crash"] = after
            summary["crash_png"] = screenshot(game, out / "crash.png")
            if after["health"] >= before["health"]:
                raise AssertionError(f"car health {before['health']} -> {after['health']}: the crash cost nothing")
            if after["position"][2] >= WALL_FACE_Z - half_z + 0.3:
                raise AssertionError(f"car centre z {after['position'][2]:.2f} passed the wall face")
            if speed(after["velocity"]) >= STOPPED:
                raise AssertionError(f"car still moving at {speed(after['velocity']):.2f} m/s")

            # 4. F: on foot next to the car.
            game.send_keys(["KeyF"], 100)
            poll("on foot", lambda: driving(game) is None, 2, 0.05)
            time.sleep(0.3)
            me = player(game)
            reach = flat(me["position"], car(game, target["entity"])["position"])
            summary["exit"] = {"distance": reach, "health": me["health"], "state": game_state(game)}
            if reach > EXIT_REACH:
                raise AssertionError(f"exited {reach:.2f} m from the car")
            if summary["exit"]["state"] != "Playing":
                raise AssertionError(f"GameState {summary['exit']['state']} after the exit")
            summary["exit_png"] = screenshot(game, out / "exit.png")
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
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t14")
    run(parser.parse_args().out)


if __name__ == "__main__":
    main()
