"""Runtime T8 gate: civilians. The street is populated from the first second after loading (some
civilians closer than the 60 m spawn ring), the bubble fills to the cap, and one pistol shot into the
sky scatters the civilians within hearing range (share of Flee + Cower rises).
Hard pass/fail rests on the numbers; screenshots and diagnostics are evidence for the owner."""

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
from t5 import game_state, log_errors, resource_value, screenshot, wait_chunks  # noqa: E402
from t6 import camera_config, player, rows, teleport, weapon_pickups  # noqa: E402

SEED = 1
FIRST_FILL_S = 1.0
FILL_DEADLINE_S = 120.0
SETTLE_S = 0.5
STAND_OFF_M = 1.5
SKY_PITCH_DEG = 60.0
CLICK_MS = 80
# One perception cycle is 4 fixed ticks (62.5 ms); the rest is BRP latency.
REACTION_WAIT_S = 0.3
SCARED = ("Flee", "Cower")


def ron_text(rel):
    return (REPO / "assets" / rel).read_text(encoding="utf-8")


def ron_number(text, pattern):
    match = re.search(pattern, text, re.S)
    if not match:
        raise AssertionError(f"GATE BROKEN: {pattern!r} not found")
    return float(match.group(1))


def population_config():
    text = ron_text("npc/population.ron")
    return {
        "spawn_ring_inner": ron_number(text, r"spawn_ring:\s*\(\s*([\d.]+)"),
        "max_civilians": int(ron_number(text, r"max_civilians:\s*(\d+)")),
    }


def hearing_radius():
    return ron_number(ron_text("npc/perception.ron"), r"hearing_radius:\s*([\d.]+)")


def civilian_model_count():
    block = re.search(r"civilian_models:\s*\[(.*?)\]", ron_text("character/visual.ron"), re.S)
    if not block:
        raise AssertionError("GATE BROKEN: civilian_models not found in visual.ron")
    return len(re.findall(r"\"[^\"]+\.glb\"", block.group(1)))


def variant(raw):
    """Reflected enum: "Wander" or {"Flee": {...}}."""
    if isinstance(raw, str):
        return raw
    if isinstance(raw, dict) and len(raw) == 1:
        return next(iter(raw))
    raise AssertionError(f"unexpected enum value {raw!r}")


def scalar(raw):
    while isinstance(raw, (list, dict)):
        raw = raw[0] if isinstance(raw, list) else next(iter(raw.values()))
    return raw


def civilians(game):
    found = rows(game, ["Civilian", "Position", "Appearance"])
    return [
        {"entity": e, "state": variant(c["state"]), "position": vec3(p), "appearance": int(scalar(a))}
        for e, (c, p, a) in found
    ]


def alive(people):
    return [c for c in people if c["state"] != "Dead"]


def horizontal(a, b):
    return math.hypot(a[0] - b[0], a[2] - b[2])


def counts(people):
    result = {}
    for c in people:
        result[c["state"]] = result.get(c["state"], 0) + 1
    return result


def scared_share(people):
    return sum(1 for c in people if c["state"] in SCARED) / len(people) if people else 0.0


def near(people, at, radius):
    return [c for c in people if horizontal(c["position"], at) <= radius]


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    check = subprocess.run([sys.executable, str(REPO / "tools" / "fetch_assets.py"), "--check"], cwd=REPO)
    if check.returncode != 0:
        raise AssertionError("run python tools/fetch_assets.py first")
    population = population_config()
    hearing = hearing_radius()
    models = civilian_model_count()
    cam = camera_config()
    summary = {"seed": SEED, "population": population, "hearing_radius": hearing, "civilian_models": models}
    try:
        with Game(features=("dev",), args=("--seed", str(SEED)), release=True) as game:
            # 1. First fill: within a second of entering Playing, civilians already stand nearby.
            deadline = time.monotonic() + 180
            while True:
                try:
                    if game_state(game) == "Playing":
                        break
                except RuntimeError:
                    pass
                if time.monotonic() > deadline:
                    raise AssertionError("GameState never became Playing")
                time.sleep(0.05)
            playing_at = time.monotonic()
            first = []
            closer = []
            while time.monotonic() - playing_at < FIRST_FILL_S:
                me = player(game)
                first = alive(civilians(game))
                closer = [c for c in first if horizontal(c["position"], me["position"]) < population["spawn_ring_inner"]]
                if closer:
                    break
                time.sleep(0.05)
            summary["first_fill"] = {
                "after_s": round(time.monotonic() - playing_at, 3),
                "count": len(first),
                "closer_than_ring": len(closer),
                "screenshot": screenshot(game, out / "first_fill.png"),
            }
            if not closer:
                raise AssertionError(f"no civilian closer than {population['spawn_ring_inner']} m within {FIRST_FILL_S} s: {summary['first_fill']}")
            layout_hash = resource_value(game, "CityLayoutHash")
            layout_hash = int(scalar(layout_hash))
            if layout_hash != load_golden()[SEED]:
                raise AssertionError(f"layout hash {layout_hash:#x} is not the golden hash of seed {SEED}")
            summary["chunks"] = wait_chunks(game)

            # 2. The bubble fills to the cap.
            target = population["max_civilians"]
            while True:
                people = alive(civilians(game))
                if len(people) >= target:
                    break
                if time.monotonic() - playing_at > FILL_DEADLINE_S:
                    raise AssertionError(f"only {len(people)} of {target} civilians after {FILL_DEADLINE_S} s")
                time.sleep(0.5)
            per_model = {}
            for c in people:
                key = 1 + c["appearance"] % models
                per_model[key] = per_model.get(key, 0) + 1
            summary["crowd"] = {
                "time_to_cap_s": round(time.monotonic() - playing_at, 1),
                "count": len(people),
                "states": counts(people),
                "per_model": per_model,
                "diagnostics": game.diagnostics(),
                "screenshot": screenshot(game, out / "crowd.png"),
            }
            if len(per_model) < 2:
                raise AssertionError(f"civilians use fewer than 2 models: {per_model}")

            # 3. Pistol.
            me = player(game)
            gun = next(i for i in weapon_pickups(game) if i["weapon"] == "Pistol" and not i["ammo_only"])
            teleport(game, me, gun["at"])
            time.sleep(SETTLE_S)
            me = player(game)
            if me["held"] != "Pistol":
                raise AssertionError(f"pistol not picked up: {me['held']}")

            # 4. Stand next to the densest group.
            people = alive(civilians(game))
            anchor = max(people, key=lambda c: len(near(people, c["position"], hearing)))
            group = near(people, anchor["position"], hearing)
            cx = sum(c["position"][0] for c in group) / len(group)
            cz = sum(c["position"][2] for c in group) / len(group)
            ax, ay, az = anchor["position"]
            dx, dz = cx - ax, cz - az
            length = math.hypot(dx, dz) or 1.0
            feet = [ax + dx / length * STAND_OFF_M, ay - me["float_height"], az + dz / length * STAND_OFF_M]
            teleport(game, me, feet)
            time.sleep(SETTLE_S)

            # 5. One shot into the sky.
            me = player(game)
            before_people = near(alive(civilians(game)), me["position"], hearing)
            if not before_people:
                raise AssertionError("no civilian within hearing range of the shot")
            magazine = me["guns"][0]["magazine"]
            game.move_mouse(0, -(SKY_PITCH_DEG / cam["sensitivity_deg"]))
            time.sleep(0.2)
            game.send_mouse_button("Left", CLICK_MS)
            time.sleep(REACTION_WAIT_S)
            fired = player(game)["guns"][0]["magazine"]
            ids = {c["entity"] for c in before_people}
            after_people = [c for c in civilians(game) if c["entity"] in ids and c["state"] != "Dead"]
            time.sleep(0.15)
            summary["shot"] = {
                "magazine": (magazine, fired),
                "in_hearing": len(before_people),
                "before": counts(before_people),
                "after": counts(after_people),
                "scared_share": (round(scared_share(before_people), 3), round(scared_share(after_people), 3)),
                "perception_load": resource_value(game, "PerceptionLoad"),
                "screenshot": screenshot(game, out / "scatter.png"),
            }
            if fired != magazine - 1:
                raise AssertionError(f"the pistol did not fire: magazine {magazine} -> {fired}")
            if scared_share(after_people) <= scared_share(before_people):
                raise AssertionError(f"the shot did not scare anyone: {summary['shot']}")

            # 6. Clean log, clean exit.
            summary["game_state"] = game_state(game)
            errors = log_errors(game)
            summary["log_errors"] = errors
            if errors:
                raise AssertionError("errors in the game log:\n" + "\n".join(errors))
            game.shutdown()
            game.process.wait(timeout=15)
    finally:
        (out / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(summary, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t8")
    run(parser.parse_args().out.resolve())
