"""Runtime T7 gate: on the park range the unarmed player punches a dummy three times with the left
button (combo: jab, jab, knockdown finisher), the dummy's reflected HitReaction goes KnockedDown and
back to Steady, its health drops by the three fist damages of melee.ron and live damage numbers show
the punches; then the bat is picked up, key 1 toggles fists/bat, and one bat swing knocks another
dummy down. Hard pass/fail rests on the numbers; screenshots are evidence for the owner."""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from brp import Game, REPO, load_golden, vec3  # noqa: E402
from t5 import game_state, log_errors, screenshot, wait_chunks  # noqa: E402
from t6 import aim_at, camera_config, damage_numbers, dummies, dummy, rows, teleport, weapon_name  # noqa: E402

SEED = 1
SETTLE_S = 1.0
CHEST_M = 1.0
STAND_OFF_M = 1.0
AIM_BEYOND_M = 20.0
CLICK_MS = 80
CLICK_GAP_S = 0.3
KNOCKDOWN_DEADLINE_S = 2.0


def melee_table():
    text = (REPO / "assets" / "combat" / "melee.ron").read_text(encoding="utf-8")
    knockdown = re.search(r"^\s*knockdown:\s*([\d.]+)", text, re.M)
    fists = re.search(r"fists:\s*\(.*?hits:\s*\[(.*?)\]\s*\)", text, re.S)
    bat = re.search(r"bat:\s*\(.*?hits:\s*\[(.*?)\]\s*\)", text, re.S)
    if not (knockdown and fists and bat):
        raise AssertionError("GATE BROKEN: melee.ron layout not understood")
    damage = lambda hits: [int(d) for d in re.findall(r"damage:\s*(\d+)", hits)]  # noqa: E731
    return {"knockdown": float(knockdown.group(1)), "fists": damage(fists.group(1)), "bat": damage(bat.group(1))}


def reaction_name(raw):
    """Tolerant reader of a reflected `HitReaction`: "Steady" or {"KnockedDown": {"left": ..}}."""
    if isinstance(raw, str):
        return raw
    if isinstance(raw, dict) and len(raw) == 1:
        return next(iter(raw))
    raise AssertionError(f"unexpected HitReaction value {raw!r}")


def reaction(game, entity):
    for e, (_, value) in rows(game, ["Dummy", "HitReaction"]):
        if e == entity:
            return reaction_name(value)
    raise AssertionError(f"dummy {entity} has no HitReaction row")


def me(game):
    found = rows(game, ["Position", "CharacterBody", "Loadout"], with_="Player")
    if len(found) != 1:
        raise AssertionError(f"expected one Player row, got {len(found)}")
    entity, (position, body, loadout) = found[0]
    return {
        "entity": entity,
        "position": vec3(position),
        "float_height": body["float_height"],
        "held": weapon_name(loadout["held"]),
        "melee": loadout["melee"],
        "has_bat": loadout["has_bat"],
    }


def feet_of(target, float_height):
    x, y, z = target["position"]
    return [x, y - float_height, z]


def face_dummy(game, target, sensitivity_deg):
    """Teleport the player 1 m in front (+Z) of the dummy and aim along -Z through it.

    Only the aim yaw steers a swing; a point 1 m away is too close for the shoulder camera to
    converge on, so the aim point lies `AIM_BEYOND_M` further down the same line."""
    player = me(game)
    feet = feet_of(target, player["float_height"])
    teleport(game, player, [feet[0], feet[1], feet[2] + STAND_OFF_M])
    time.sleep(SETTLE_S)
    return aim_at(game, [feet[0], feet[1] + CHEST_M, feet[2] - AIM_BEYOND_M], sensitivity_deg)


def wait_reaction(game, entity, wanted, deadline_s):
    start = time.monotonic()
    seen = []
    while time.monotonic() - start < deadline_s:
        name = reaction(game, entity)
        if not seen or seen[-1] != name:
            seen.append(name)
        if name == wanted:
            return round(time.monotonic() - start, 3), seen
        time.sleep(0.02)
    raise AssertionError(f"dummy {entity} never became {wanted} in {deadline_s} s: {seen}")


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    check = subprocess.run([sys.executable, str(REPO / "tools" / "fetch_assets.py"), "--check"], cwd=REPO)
    if check.returncode != 0:
        raise AssertionError("run python tools/fetch_assets.py first")
    table = melee_table()
    cam = camera_config()
    summary = {"seed": SEED, "melee": table}
    try:
        with Game(features=("dev",), args=("--seed", str(SEED)), release=True) as game:
            layout_hash = game.wait_resource("CityLayoutHash", 180)
            if layout_hash != load_golden()[SEED]:
                raise AssertionError(f"layout hash {layout_hash:#x} is not the golden hash of seed {SEED}")
            summary["chunks"] = wait_chunks(game)
            targets = dummies(game)
            if len(targets) != 3:
                raise AssertionError(f"expected 3 dummies, got {len(targets)}")

            # 1. Unarmed in front of the middle dummy.
            player = me(game)
            summary["start"] = {"held": player["held"], "melee": player["melee"], "has_bat": player["has_bat"]}
            if player["held"] is not None or player["melee"] != "Fists":
                raise AssertionError(f"player does not start unarmed with fists: {summary['start']}")
            middle = targets[1]
            summary["combo_aim_miss_m"] = round(face_dummy(game, middle, cam["sensitivity_deg"]), 3)
            before = dummy(game, middle["entity"])["health"]

            # 2. Three clicks 0.3 s apart; screenshots between and after (>= 0.15 s apart).
            start = time.monotonic()
            at = lambda t: time.sleep(max(0.0, start + t - time.monotonic()))  # noqa: E731
            clicks, shots = [], []
            for i in range(3):
                at(i * CLICK_GAP_S)
                game.send_mouse_button("Left", CLICK_MS)
                clicks.append(round(time.monotonic() - start, 3))
                if i == 1:
                    at(0.45)
                    shots.append(screenshot(game, out / "combo_1.png"))
            numbers = [n["value"] for n in damage_numbers(game)]
            at(0.9)
            shots.append(screenshot(game, out / "combo_2.png"))
            numbers += [n["value"] for n in damage_numbers(game)]

            # 3. Knockdown, damage, liveness of the melee damage numbers, then standing up.
            deadline = KNOCKDOWN_DEADLINE_S - (time.monotonic() - start)
            down_after, seen = wait_reaction(game, middle["entity"], "KnockedDown", max(deadline, 0.05))
            at(1.2)
            shots.append(screenshot(game, out / "knockdown.png"))
            numbers += [n["value"] for n in damage_numbers(game)]
            after = dummy(game, middle["entity"])["health"]
            up_after, _ = wait_reaction(game, middle["entity"], "Steady", table["knockdown"] + 1.0)
            summary["combo"] = {
                "clicks_s": clicks, "reactions": seen, "knocked_down_after_s": down_after,
                "steady_again_after_s": up_after, "health": (before, after),
                "damage_numbers": numbers, "screenshots": shots,
            }
            expected = sum(table["fists"])
            if before - after != expected:
                raise AssertionError(f"combo health drop {before - after}, expected {expected}")
            if not any(v in table["fists"] for v in numbers):
                raise AssertionError(f"no live melee damage number in {set(table['fists'])}: {numbers}")

            # 4. Bat: pick up, key 1 toggles, one swing knocks another dummy down.
            bat_rows = [vec3(t["translation"]) for _, (_, t) in rows(game, ["BatPickup", "Transform"])]
            if len(bat_rows) != 1:
                raise AssertionError(f"expected one bat pickup, got {bat_rows}")
            player = me(game)
            teleport(game, player, bat_rows[0])
            time.sleep(0.5)
            picked = me(game)
            toggles = []
            for _ in range(2):
                game.send_keys(["Digit1"], 100)
                time.sleep(0.4)
                toggles.append(me(game)["melee"])
            summary["bat_pickup"] = {"has_bat": picked["has_bat"], "melee": picked["melee"], "toggles": toggles}
            if not picked["has_bat"] or picked["melee"] != "Bat":
                raise AssertionError(f"bat not picked up: {summary['bat_pickup']}")
            if toggles != ["Fists", "Bat"]:
                raise AssertionError(f"key 1 did not toggle fists/bat: {toggles}")
            side = targets[0]
            summary["bat_aim_miss_m"] = round(face_dummy(game, side, cam["sensitivity_deg"]), 3)
            before = dummy(game, side["entity"])["health"]
            game.send_mouse_button("Left", CLICK_MS)
            bat_down_after, _ = wait_reaction(game, side["entity"], "KnockedDown", KNOCKDOWN_DEADLINE_S)
            time.sleep(0.15)
            bat_png = screenshot(game, out / "bat_knockdown.png")
            after = dummy(game, side["entity"])["health"]
            summary["bat_swing"] = {
                "knocked_down_after_s": bat_down_after, "health": (before, after), "screenshot": bat_png,
            }
            if before - after != sum(table["bat"]):
                raise AssertionError(f"bat health drop {before - after}, expected {sum(table['bat'])}")

            # 5. Clean log, clean exit.
            summary["game_state"] = game_state(game)
            errors = log_errors(game)
            summary["log_errors"] = errors
            if errors:
                raise AssertionError("font/asset errors in the game log:\n" + "\n".join(errors))
            game.shutdown()
            game.process.wait(timeout=15)
    finally:
        (out / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(summary, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t7")
    run(parser.parse_args().out.resolve())
