"""Runtime T6 gate: pick up the pistol on the park range, aim at a dummy with the mouse, fire with
the left button and read the dummy's Health, the player's ammo and the floating damage numbers
(body hit and red CRIT headshot); then an aimed SMG burst, a reload, a shotgun blast (one summed
number per blast and target) and the no-leak check.
Hard pass/fail rests on the numbers; screenshots are evidence for the owner."""

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

SEED = 1
SETTLE_S = 0.5
AIM_TOLERANCE_M = 0.1
CHEST_M = 1.0
HEAD_M = 1.7


def ron_number(text, pattern):
    match = re.search(pattern, text, re.S)
    if not match:
        raise AssertionError(f"GATE BROKEN: {pattern!r} not found")
    return float(match.group(1))


def weapon_table():
    text = (REPO / "assets" / "combat" / "weapons.ron").read_text(encoding="utf-8")
    table = {"headshot_multiplier": ron_number(text, r"headshot_multiplier:\s*([\d.]+)")}
    for name in ("pistol", "smg", "shotgun"):
        table[name] = {
            field: ron_number(text, rf"{name}:\s*\(.*?{field}:\s*([\d.]+)")
            for field in ("damage", "damage_variance", "fire_interval", "magazine", "reload")
        }
    return table


def camera_config():
    text = (REPO / "assets" / "camera" / "camera.ron").read_text(encoding="utf-8")
    return {
        "sensitivity_deg": ron_number(text, r"mouse_sensitivity_deg:\s*([\d.]+)"),
        "aim_scale": ron_number(text, r"aim_sensitivity_scale:\s*([\d.]+)"),
    }


def roll(base, variance, u):
    """`gta_sim::combat::roll_damage`: round half away from zero, at least 1."""
    return max(1, math.floor(base * (1.0 + variance * (2.0 * u - 1.0)) + 0.5))


def band(base, variance):
    return roll(base, variance, 0.0), roll(base, variance, 1.0)


def weapon_name(raw):
    """Tolerant reader of a reflected `Option<Weapon>`: "None", null, {"Some": x} or a bare name."""
    if raw is None or raw == "None":
        return None
    if isinstance(raw, dict) and len(raw) == 1:
        return weapon_name(next(iter(raw.values())))
    if isinstance(raw, list) and len(raw) == 1:
        return weapon_name(raw[0])
    return raw


def rows(game, components, with_=None):
    paths = [game.component_path(name) for name in components]
    marker = game.component_path(with_) if with_ else paths[0]
    return [
        (row["entity"], [row["components"][p] for p in paths])
        for row in game.query(paths, with_=[marker])
    ]


def player(game):
    found = rows(game, ["Position", "CharacterBody", "Loadout", "AimIntent"], with_="Player")
    if len(found) != 1:
        raise AssertionError(f"expected one Player row, got {len(found)}")
    entity, (position, body, loadout, aim) = found[0]
    return {
        "entity": entity,
        "position": vec3(position),
        "float_height": body["float_height"],
        "held": weapon_name(loadout["held"]),
        "guns": loadout["guns"],
        "aim": {"origin": vec3(aim["origin"]), "direction": vec3(aim["direction"]), "aiming": aim["aiming"]},
    }


def dummies(game):
    found = rows(game, ["Dummy", "Position", "Health"])
    return sorted(
        ({"entity": e, "position": vec3(p), "health": h["current"]} for e, (_, p, h) in found),
        key=lambda d: d["position"][0],
    )


def dummy(game, entity):
    return next(d for d in dummies(game) if d["entity"] == entity)


def is_dead(game, entity):
    dead = game.component_path("Dead")
    return any(row["entity"] == entity for row in game.query([dead], with_=[dead]))


def weapon_pickups(game):
    return [
        {"weapon": p["weapon"], "ammo_only": p["ammo_only"], "at": vec3(t["translation"])}
        for _, (p, t) in rows(game, ["WeaponPickup", "Transform"])
    ]


def damage_numbers(game):
    return [n for _, (n,) in rows(game, ["DamageNumber"])]


def tracers(game):
    """Live tracer meshes (`src/vfx`, `Name("Tracer")`)."""
    name = game.component_path("Name")
    return sum(1 for row in game.query([name]) if row["components"][name] == "Tracer")


def burst_capture(game, path):
    """Screenshot bracketed by tracer counts: a capture taken while tracers exist on both sides."""
    before = tracers(game)
    png = screenshot(game, path)
    return {"screenshot": png, "tracers_before": before, "tracers_after": tracers(game)}


def teleport(game, me, feet):
    target = [feet[0], feet[1] + me["float_height"], feet[2]]
    game.mutate_component(me["entity"], game.component_path("Position"), "", target)


def yaw_pitch(direction):
    x, y, z = direction
    return math.atan2(-x, -z), math.asin(max(-1.0, min(1.0, y)))


def miss_distance(origin, direction, target):
    """Distance from `target` to the aim ray."""
    d = [t - o for t, o in zip(target, origin)]
    along = sum(a * b for a, b in zip(d, direction))
    closest = [o + a * along for o, a in zip(origin, direction)]
    return math.dist(closest, target)


def aim_at(game, target, sensitivity_deg, tries=6):
    """Turn the camera with `move_mouse` until the player's aim ray passes within tolerance of `target`."""
    sensitivity = math.radians(sensitivity_deg)
    for _ in range(tries):
        aim = player(game)["aim"]
        miss = miss_distance(aim["origin"], aim["direction"], target)
        if miss < AIM_TOLERANCE_M:
            return miss
        wanted = [t - o for t, o in zip(target, aim["origin"])]
        length = math.sqrt(sum(v * v for v in wanted))
        yaw_t, pitch_t = yaw_pitch([v / length for v in wanted])
        yaw, pitch = yaw_pitch(aim["direction"])
        d_yaw = (yaw_t - yaw + math.pi) % (2 * math.pi) - math.pi
        # apply_mouse_look: yaw -= dx * s, pitch -= dy * s.
        game.move_mouse(-d_yaw / sensitivity, -(pitch_t - pitch) / sensitivity)
        time.sleep(0.3)
    aim = player(game)["aim"]
    miss = miss_distance(aim["origin"], aim["direction"], target)
    if miss >= AIM_TOLERANCE_M:
        raise AssertionError(f"aim ray still {miss:.3f} m from {target}")
    return miss


def shot(game, target, sensitivity_deg, dummy_entity, want_head, out_png, cooldown):
    """Aim, click once and read the health drop, magazine and damage numbers of the wanted zone."""
    miss = aim_at(game, target, sensitivity_deg)
    time.sleep(cooldown)
    before = dummy(game, dummy_entity)["health"]
    mag_before = player(game)["guns"][0]["magazine"]
    game.send_mouse_button("Left", 100)
    # Tracer and hit marker last 0.1 s (juice.ron, strings.ron): capture as early as the shot can have happened.
    time.sleep(0.04)
    png = screenshot(game, out_png)
    time.sleep(0.2)
    after = dummy(game, dummy_entity)["health"]
    numbers = [n for n in damage_numbers(game) if n["headshot"] == want_head]
    return {
        "aim_miss_m": round(miss, 3),
        "health_before": before,
        "health_after": after,
        "drop": before - after,
        "magazine": (mag_before, player(game)["guns"][0]["magazine"]),
        "numbers": numbers,
        "all_numbers": damage_numbers(game),
        "screenshot": png,
    }


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    check = subprocess.run([sys.executable, str(REPO / "tools" / "fetch_assets.py"), "--check"], cwd=REPO)
    if check.returncode != 0:
        raise AssertionError("run python tools/fetch_assets.py first")
    table = weapon_table()
    cam = camera_config()
    pistol = table["pistol"]
    body_band = band(pistol["damage"], pistol["damage_variance"])
    head_band = band(pistol["damage"] * table["headshot_multiplier"], pistol["damage_variance"])
    summary = {"seed": SEED, "body_band": body_band, "head_band": head_band}
    try:
        with Game(features=("dev",), args=("--seed", str(SEED)), release=True) as game:
            layout_hash = game.wait_resource("CityLayoutHash", 180)
            if layout_hash != load_golden()[SEED]:
                raise AssertionError(f"layout hash {layout_hash:#x} is not the golden hash of seed {SEED}")
            summary["chunks"] = wait_chunks(game)
            park = vec3(resource_value(game, "CityLandmarks")["park_center"])
            summary["park_center"] = park
            targets = dummies(game)
            summary["dummies"] = targets
            if len(targets) != 3:
                raise AssertionError(f"expected 3 dummies, got {len(targets)}")
            items = weapon_pickups(game)
            summary["weapon_pickups"] = items
            if len(items) != 6:
                raise AssertionError(f"expected 6 weapon pickups, got {len(items)}")

            # 1. Pistol pickup.
            me = player(game)
            gun = next(i for i in items if i["weapon"] == "Pistol" and not i["ammo_only"])
            teleport(game, me, gun["at"])
            time.sleep(SETTLE_S)
            me = player(game)
            summary["after_pistol_pickup"] = {"held": me["held"], "guns": me["guns"]}
            if not me["guns"][0]["owned"] or me["held"] != "Pistol":
                raise AssertionError(f"pistol not picked up: {me['held']} {me['guns']}")

            # 2. Stand 5 m in front (+Z) of the middle dummy.
            target = targets[1]
            feet = list(target["position"])
            feet[1] -= me["float_height"]
            teleport(game, me, [feet[0], feet[1], feet[2] + 5.0])
            time.sleep(1.0)
            chest = [feet[0], feet[1] + CHEST_M, feet[2]]
            head = [feet[0], feet[1] + HEAD_M, feet[2]]

            # 3. Body shot.
            body = shot(game, chest, cam["sensitivity_deg"], target["entity"], False,
                        out / "body_hit.png", 0.0)
            summary["body_shot"] = body
            if body["magazine"][1] != int(pistol["magazine"]) - 1:
                raise AssertionError(f"magazine after one shot: {body['magazine']}")
            if len(body["numbers"]) != 1:
                raise AssertionError(f"expected one body damage number, got {body['all_numbers']}")
            value = body["numbers"][0]["value"]
            if not body_band[0] <= body["drop"] <= body_band[1] or body["drop"] != value:
                raise AssertionError(f"body hit: drop {body['drop']}, number {value}, band {body_band}")

            # 4. Head shot (re-aim once more if the pellet lands on the body).
            for attempt in range(3):
                crit = shot(game, head, cam["sensitivity_deg"], target["entity"], True,
                            out / "crit_hit.png", pistol["fire_interval"] + 1.0)
                summary[f"head_shot_{attempt}"] = crit
                if crit["numbers"]:
                    break
            else:
                raise AssertionError("no headshot in 3 tries")
            value = crit["numbers"][0]["value"]
            if not head_band[0] <= crit["drop"] <= head_band[1] or crit["drop"] != value:
                raise AssertionError(f"head hit: drop {crit['drop']}, number {value}, band {head_band}")

            # 5. SMG: pick up, select with key 3, aimed burst at a fresh dummy (hit markers all burst long).
            fresh = targets[2]
            fresh_feet = list(fresh["position"])
            fresh_feet[1] -= me["float_height"]
            fresh_chest = [fresh_feet[0], fresh_feet[1] + CHEST_M, fresh_feet[2]]
            me = player(game)
            smg = next(i for i in items if i["weapon"] == "Smg" and not i["ammo_only"])
            teleport(game, me, smg["at"])
            time.sleep(SETTLE_S)
            game.send_keys(["Digit3"], 100)
            time.sleep(SETTLE_S)
            teleport(game, me, [fresh_feet[0], fresh_feet[1], fresh_feet[2] + 5.0])
            time.sleep(1.0)
            me = player(game)
            if me["held"] != "Smg":
                raise AssertionError(f"key 3 did not select the SMG: {me['held']}")
            game.send_mouse_button("Right", 4000)
            time.sleep(0.4)
            aim_at(game, fresh_chest, cam["sensitivity_deg"] * cam["aim_scale"])
            aiming = player(game)["aim"]["aiming"]
            before = dummy(game, fresh["entity"])["health"]
            mag_before = player(game)["guns"][1]["magazine"]
            game.send_mouse_button("Left", 1200)
            time.sleep(0.25)
            burst_1 = burst_capture(game, out / "aim_burst_1.png")
            time.sleep(0.25)
            burst_2 = burst_capture(game, out / "aim_burst_2.png")
            time.sleep(1.0)
            me = player(game)
            after = dummy(game, fresh["entity"])["health"]
            dead = is_dead(game, fresh["entity"])
            summary["smg_burst"] = {
                "aiming": aiming, "magazine": (mag_before, me["guns"][1]["magazine"]),
                "health": (before, after), "dead": dead, "screenshots": [burst_1, burst_2],
            }
            if not aiming:
                raise AssertionError("AimIntent.aiming was false while RMB was held")
            if not me["guns"][1]["magazine"] < mag_before - 1:
                raise AssertionError(f"SMG burst fired at most one round: {mag_before} -> {me['guns'][1]['magazine']}")
            if not (after < before or dead):
                raise AssertionError(f"SMG burst did not hurt the dummy: {before} -> {after}")
            # Liveness only: the PNGs themselves are reviewed by eye (aim framing, tracer look).
            if not any(c["tracers_before"] and c["tracers_after"] for c in (burst_1, burst_2)):
                raise AssertionError(f"no burst capture was taken while tracers were alive: {burst_1} {burst_2}")

            # 6. Reload.
            game.send_keys(["KeyR"], 100)
            time.sleep(table["smg"]["reload"] + 0.5)
            magazine = player(game)["guns"][1]["magazine"]
            summary["after_reload"] = magazine
            if magazine != int(table["smg"]["magazine"]):
                raise AssertionError(f"SMG magazine after reload: {magazine}")

            # 7. Shotgun blast at 4 m: one number per blast and target, equal to the health drop.
            near = targets[0]
            near_feet = list(near["position"])
            near_feet[1] -= me["float_height"]
            me = player(game)
            shotgun = next(i for i in items if i["weapon"] == "Shotgun" and not i["ammo_only"])
            teleport(game, me, shotgun["at"])
            time.sleep(SETTLE_S)
            game.send_keys(["Digit4"], 100)
            time.sleep(SETTLE_S)
            teleport(game, me, [near_feet[0], near_feet[1], near_feet[2] + 4.0])
            time.sleep(1.0)
            if player(game)["held"] != "Shotgun":
                raise AssertionError(f"key 4 did not select the shotgun: {player(game)['held']}")
            aim_at(game, [near_feet[0], near_feet[1] + CHEST_M, near_feet[2]], cam["sensitivity_deg"])
            before = dummy(game, near["entity"])["health"]
            mag_before = player(game)["guns"][2]["magazine"]
            game.send_mouse_button("Left", 100)
            time.sleep(0.04)
            png = screenshot(game, out / "shotgun_blast.png")
            time.sleep(0.2)
            after = dummy(game, near["entity"])["health"]
            numbers = damage_numbers(game)
            dead = is_dead(game, near["entity"])
            summary["shotgun_blast"] = {
                "health": (before, after), "dead": dead, "numbers": numbers, "screenshot": png,
                "magazine": (mag_before, player(game)["guns"][2]["magazine"]),
            }
            if player(game)["guns"][2]["magazine"] != mag_before - 1:
                raise AssertionError(f"shotgun magazine: {summary['shotgun_blast']['magazine']}")
            if len(numbers) != 1:
                raise AssertionError(f"expected one damage number for the blast, got {numbers}")
            value = numbers[0]["value"]
            # A lethal blast drops less health than it dealt (overkill is not clamped in the number).
            if not (before - after == value or (dead and value >= before)):
                raise AssertionError(f"shotgun: drop {before - after}, number {value}, dead {dead}")

            # 8. No leak.
            time.sleep(2.0)
            left = damage_numbers(game)
            summary["damage_numbers_left"] = len(left)
            if left:
                raise AssertionError(f"{len(left)} damage numbers still alive after 2 s")
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
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t6")
    run(parser.parse_args().out.resolve())
