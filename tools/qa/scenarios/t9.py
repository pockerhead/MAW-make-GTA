"""Runtime T9 gate: gangs. The player walks up unseen to the gang-0 HQ, a group stands there, one
pistol shot into the sky next to it puts every member within `group_radius` into `Attack` and heats
the gang; then a firefight. Hard pass/fail rests on the states; screenshots and diagnostics are
evidence for the owner. The player fires only into the sky, so every member must end the fight at full
health: any loss is a groupmate's bullet or punch (friendly fire). `GangHeat` counts fixed-time seconds (slower in wall-clock time during the
Wasted slow motion). Named QA mutation: right before the shot the player's armour is raised so the
firefight is captured with the player alive (lethality is the owner's call, not this gate's)."""

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
from t5 import game_state, resource_value, screenshot, wait_chunks  # noqa: E402
from t6 import camera_config, player, rows, teleport, weapon_name  # noqa: E402
from t8 import horizontal, scalar, variant  # noqa: E402

SEED = 1
SETTLE_S = 0.5
GROUP_DEADLINE_S = 15.0
STAND_OFF_M = 12.0
CLOSER_STEP_M = 2.0
CLOSER_TRIES = 3
SKY_PITCH_DEG = 60.0
CLICK_MS = 80
# One fixed tick provokes the group; the rest is BRP latency.
AGGRO_WAIT_S = 0.3
FIREFIGHT_S = 6.0
FIREFIGHT_POLL_S = 0.25
# A car body this close to a member when its health drops ran it over (T15 traffic), m.
RUN_OVER_M = 5.0
FIGHT_ARMOR = 1.0e6
HEAT_FLOOR_S = 110.0
CAPTURE_GAP_S = 0.15


def ron_text(rel):
    return (REPO / "assets" / rel).read_text(encoding="utf-8")


def ron_number(text, pattern):
    match = re.search(pattern, text, re.S)
    if not match:
        raise AssertionError(f"GATE BROKEN: {pattern!r} not found")
    return float(match.group(1))


def spawn_ring():
    text = ron_text("npc/population.ron")
    return (
        ron_number(text, r"spawn_ring:\s*\(\s*([\d.]+)"),
        ron_number(text, r"spawn_ring:\s*\(\s*[\d.]+\s*,\s*([\d.]+)"),
    )


def group_radius():
    return ron_number(ron_text("gang/gangs.ron"), r"group_radius:\s*([\d.]+)")


def max_health():
    return ron_number(ron_text("character/health.ron"), r"max_health:\s*([\d.]+)")


def option(raw):
    """Reflected `Option<T>`: "None", null, {"Some": x}, [x] or a bare value."""
    if raw is None or raw == "None":
        return None
    if isinstance(raw, dict) and len(raw) == 1:
        return option(next(iter(raw.values())))
    if isinstance(raw, list):
        return option(raw[0]) if raw else None
    return raw


def gang_posts(game, gang):
    territories = resource_value(game, "GangTerritories")
    return [vec3(p) for p in territories["gangs"][gang]["posts"]]


def player_territory(game):
    value = option(resource_value(game, "PlayerTerritory"))
    return None if value is None else int(value)


def heat(game):
    return [float(v) for v in resource_value(game, "GangHeat")["left"]]


def members(game):
    found = rows(game, ["GangMember", "Position", "Loadout"])
    return [
        {
            "entity": e,
            "gang": int(m["gang"]),
            "post": int(m["post"]),
            "state": variant(m["state"]),
            "position": vec3(p),
            "held": weapon_name(l["held"]),
        }
        for e, (m, p, l) in found
    ]


def pistol_pickups(game):
    dropped = {row["entity"] for row in game.query([game.component_path("Dropped")])}
    return [
        vec3(t["translation"])
        for e, (p, t) in rows(game, ["WeaponPickup", "Transform"])
        if p["weapon"] == "Pistol" and not p["ammo_only"] and e not in dropped
    ]


def camera(game):
    found = rows(game, ["OrbitCamera"])
    if len(found) != 1:
        raise AssertionError(f"expected one OrbitCamera, got {len(found)}")
    return found[0][0]


def look_along(game, d):
    """Turns the orbit camera to look along the flat direction `d` (GDD §3.2 yaw convention)."""
    game.mutate_component(camera(game), game.component_path("OrbitCamera"), ".yaw", math.atan2(-d[0], -d[2]))


def health(game):
    found = rows(game, ["Health"], with_="Player")
    return found[0][1][0]


def rounds(guns):
    """Rounds a gang member carries; a drop between two reads is shots fired (a reload moves rounds, never
    loses them)."""
    return sum(g["magazine"] + g["reserve"] for g in guns)


def fighters(game, ids):
    found = rows(game, ["GangMember", "Loadout", "Health"])
    return {
        e: {"state": variant(m["state"]), "gun": m["gun"], "rounds": rounds(l["guns"]), "health": h["current"]}
        for e, (m, l, h) in found
        if e in ids
    }


def log_error_lines(game):
    return [line for line in game.log_tail(2_000_000).splitlines() if "ERROR" in line]


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    check = subprocess.run([sys.executable, str(REPO / "tools" / "fetch_assets.py"), "--check"], cwd=REPO)
    if check.returncode != 0:
        raise AssertionError("run python tools/fetch_assets.py first")
    inner, outer = spawn_ring()
    radius = group_radius()
    cam = camera_config()
    summary = {"seed": SEED, "spawn_ring": (inner, outer), "group_radius": radius}
    try:
        with Game(features=("dev",), args=("--seed", str(SEED)), release=True) as game:
            # 1. Load.
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
            layout_hash = int(scalar(resource_value(game, "CityLayoutHash")))
            if layout_hash != load_golden()[SEED]:
                raise AssertionError(f"layout hash {layout_hash:#x} is not the golden hash of seed {SEED}")
            summary["chunks"] = wait_chunks(game)

            # 2. Territories.
            posts = gang_posts(game, 0)
            hq = posts[0]
            ring = [p for p in posts[1:] if inner <= horizontal(p, hq) <= outer]
            if not ring:
                raise AssertionError("GATE BROKEN: no gang-0 post in the spawn ring of the HQ")
            approach = min(ring, key=lambda p: horizontal(p, hq))
            summary["territory"] = {"posts": len(posts), "hq": hq, "approach": approach,
                                    "approach_to_hq_m": round(horizontal(approach, hq), 1)}

            # 3. Pistol from the range (not a dropped gun).
            me = player(game)
            guns = pistol_pickups(game)
            if not guns:
                raise AssertionError("no pistol pickup on the range")
            teleport(game, me, guns[0])
            time.sleep(SETTLE_S)
            me = player(game)
            if me["held"] != "Pistol":
                raise AssertionError(f"pistol not picked up: {me['held']}")

            # 4. Walk up unseen: stand on the approach post looking away from the HQ.
            teleport(game, me, approach)
            look_along(game, [approach[i] - hq[i] for i in range(3)])
            deadline = time.monotonic() + GROUP_DEADLINE_S
            while True:
                group = [m for m in members(game) if m["gang"] == 0 and m["post"] == 0 and m["state"] != "Dead"]
                if group:
                    break
                if time.monotonic() > deadline:
                    raise AssertionError(f"no HQ group within {GROUP_DEADLINE_S} s")
                time.sleep(0.1)
            time.sleep(SETTLE_S)
            group = [m for m in members(game) if m["gang"] == 0 and m["post"] == 0 and m["state"] != "Dead"]
            cx = sum(m["position"][0] for m in group) / len(group)
            cz = sum(m["position"][2] for m in group) / len(group)
            centroid = [cx, hq[1], cz]
            summary["hq_group"] = {"size": len(group), "centroid": centroid}

            # 5. Stand 12 m from the group on the approach line, inside the territory.
            d = [approach[0] - cx, 0.0, approach[2] - cz]
            length = math.hypot(d[0], d[2]) or 1.0
            unit = [d[0] / length, 0.0, d[2] / length]
            stand = STAND_OFF_M
            for _ in range(CLOSER_TRIES + 1):
                feet = [cx + unit[0] * stand, hq[1], cz + unit[2] * stand]
                teleport(game, player(game), feet)
                time.sleep(SETTLE_S)
                if player_territory(game) == 0:
                    break
                stand -= CLOSER_STEP_M
            territory_at_shot = player_territory(game)
            look_along(game, [-unit[0], 0.0, -unit[2]])
            time.sleep(SETTLE_S)
            ids = {m["entity"] for m in group}
            before = [m for m in members(game) if m["entity"] in ids]
            summary["before"] = {
                "stand_off_m": stand,
                "player_territory": territory_at_shot,
                "states": [m["state"] for m in before],
                "screenshot": screenshot(game, out / "hq.png"),
            }
            time.sleep(CAPTURE_GAP_S)

            # 6. One shot into the sky.
            me = player(game)
            game.mutate_component(me["entity"], game.component_path("Health"), ".armor", FIGHT_ARMOR)
            magazine = me["guns"][0]["magazine"]
            game.move_mouse(0, -(SKY_PITCH_DEG / cam["sensitivity_deg"]))
            time.sleep(0.2)
            game.send_mouse_button("Left", CLICK_MS)
            time.sleep(AGGRO_WAIT_S)
            fired = player(game)["guns"][0]["magazine"]
            near = [m for m in members(game) if m["gang"] == 0 and horizontal(m["position"], centroid) <= radius]
            gang_heat = heat(game)
            summary["after"] = {
                "magazine": (magazine, fired),
                "states": {str(m["entity"]): m["state"] for m in near},
                "gang_heat": gang_heat,
                "screenshot": screenshot(game, out / "aggro.png"),
            }

            # 7. Firefight evidence, captured while the player lives and the group attacks.
            game.move_mouse(0, SKY_PITCH_DEG / cam["sensitivity_deg"])
            ids = {m["entity"] for m in near}
            start = fighters(game, ids)
            fight_t0 = time.monotonic()
            capture = None
            while capture is None and time.monotonic() - fight_t0 < FIREFIGHT_S:
                time.sleep(FIREFIGHT_POLL_S)
                now = fighters(game, ids)
                spent = sum(start[e]["rounds"] - now[e]["rounds"] for e in now if now[e]["state"] != "Dead")
                attacking = sum(1 for m in now.values() if m["state"] == "Attack")
                hp = health(game)
                if game_state(game) == "Playing" and hp["current"] > 0.0 and attacking and spent > 0:
                    capture = {
                        "t_s": round(time.monotonic() - fight_t0, 2),
                        "player_health": hp,
                        "attacking": attacking,
                        "screenshot": screenshot(game, out / "firefight.png"),
                    }
            # T15: traffic drives through the fight. A member whose health drops with a car body within
            # RUN_OVER_M is logged as run over, not as friendly fire.
            run_over = {}
            last = {e: f["health"] for e, f in fighters(game, ids).items()}
            while time.monotonic() - fight_t0 < FIREFIGHT_S:
                time.sleep(FIREFIGHT_POLL_S)
                now = fighters(game, ids)
                dropped = [e for e, f in now.items() if f["health"] < last.get(e, f["health"])]
                if dropped:
                    cars = [vec3(p) for _, (p,) in rows(game, ["Position"], with_="Vehicle")]
                    at = {m["entity"]: m["position"] for m in members(game)}
                    for e in dropped:
                        if e in at and any(horizontal(at[e], c) <= RUN_OVER_M for c in cars):
                            run_over[str(e)] = run_over.get(str(e), 0.0) + last[e] - now[e]["health"]
                last = {e: f["health"] for e, f in now.items()}
            end = fighters(game, ids)
            summary["firefight"] = {
                "armor_raised_to": FIGHT_ARMOR,
                "capture": capture,
                "armor_lost": FIGHT_ARMOR - health(game)["armor"],
                "members_over_s": FIREFIGHT_S,
                "members": {
                    str(e): {
                        "gun": start[e]["gun"],
                        "shots": start[e]["rounds"] - end[e]["rounds"] if end[e]["state"] != "Dead" else None,
                        "state": end[e]["state"],
                        "health": end[e]["health"],
                    }
                    for e in start
                    if e in end
                },
                "drawn": sum(1 for m in members(game) if m["entity"] in ids and m["held"] is not None),
                "route_load": resource_value(game, "RouteLoad"),
                "run_over": run_over,
                "frame": game.frame_report(),
            }

            # 8. Verdict.
            summary["game_state"] = game_state(game)
            errors = log_error_lines(game)
            summary["log_errors"] = errors
            if fired != magazine - 1:
                raise AssertionError(f"the pistol did not fire: magazine {magazine} -> {fired}")
            if territory_at_shot != 0:
                raise AssertionError(f"the player was not in gang 0's territory at the shot: {territory_at_shot}")
            if any(state == "Attack" for state in summary["before"]["states"]):
                raise AssertionError(f"members attacked before the shot: {summary['before']['states']}")
            if not near or any(m["state"] != "Attack" for m in near):
                raise AssertionError(f"not every member within {radius} m attacks: {summary['after']['states']}")
            if gang_heat[0] <= HEAT_FLOOR_S:
                raise AssertionError(f"GangHeat[0] {gang_heat[0]} <= {HEAT_FLOOR_S}")
            if capture is None:
                raise AssertionError(f"firefight evidence incomplete: no moment with the player alive, a member "
                                     f"attacking and a shot fired within {FIREFIGHT_S} s")
            full = max_health()
            left = summary["firefight"]["members"]
            hurt = {e: m for e, m in left.items()
                    if m["health"] < full - summary["firefight"]["run_over"].get(e, 0.0)}
            if hurt or len(left) != len(start):
                raise AssertionError(f"friendly fire: members below {full} HP after the fight: {hurt} "
                                     f"({len(start)} started, {len(left)} left)")
            if errors:
                raise AssertionError("errors in the game log:\n" + "\n".join(errors))
            game.shutdown()
            game.process.wait(timeout=15)
    finally:
        (out / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(summary, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t9")
    run(parser.parse_args().out.resolve())
