"""Runtime T10 gate: wanted level. A pistol kill next to civilians is phoned in by a hearing caller
(indicator, then stars); the player teleports far outside the search circle and the stars clear after
`clear_seconds`; a BRP heat mutation shows the star thresholds. Named QA mutations (t9 precedent): the
victim is held idle at 1 HP (the property under test is the report, not the pistol's damage roll), and
one civilian in reporting range is held idle with a report-prone temperament (the temperament roll is
not the property under test). Hard pass/fail rests on `WantedLevel` and civilian states; screenshots
are evidence for the owner."""

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
from t6 import aim_at, camera_config, player, teleport, weapon_pickups  # noqa: E402
from t8 import alive, civilians, horizontal, scalar  # noqa: E402

SEED = 1
SETTLE_S = 0.5
BUBBLE_MIN = 20
BUBBLE_DEADLINE_S = 120.0
GROUP_RADIUS_M = 40.0
GROUP_MIN = 3
VICTIM_STAND_OFF_M = 5.0
CALLER_RANGE_M = (25.0, 38.0)
TELEPORT_TOLERANCE_M = 1.0
CHEST_M = 1.0
CLICK_MS = 80
CIRCLE_MARGIN_M = 10.0
CAPTURE_GAP_S = 0.2
PLAN_TRIES = 5
CALLER_TEMPERAMENT = {"flee": 0.5, "cower": 1.0, "report": 1.5}
HOLD = {"Idle": {"left": 1.0e6}}


def ron_text(rel):
    return (REPO / "assets" / rel).read_text(encoding="utf-8")


def ron_number(text, pattern):
    match = re.search(pattern, text, re.S)
    if not match:
        raise AssertionError(f"GATE BROKEN: {pattern!r} not found")
    return float(match.group(1))


def wanted_config():
    text = ron_text("wanted/wanted.ron")
    rows = re.findall(r"\(heat:\s*(\d+),\s*search_radius:\s*([\d.]+),\s*clear_seconds:\s*([\d.]+)\)", text)
    if len(rows) != 5:
        raise AssertionError(f"GATE BROKEN: expected 5 star rows in wanted.ron, got {len(rows)}")
    return {
        "kill_person": int(ron_number(text, r"kill_person:\s*(\d+)")),
        "stars": [{"heat": int(h), "search_radius": float(r), "clear_seconds": float(c)} for h, r, c in rows],
    }


def civilian_config():
    civilian = ron_text("npc/civilian.ron")
    return {
        "call_seconds": ron_number(civilian, r"call_seconds:\s*([\d.]+)"),
        "report_min_distance": ron_number(civilian, r"report_min_distance:\s*([\d.]+)"),
        "hearing_radius": ron_number(ron_text("npc/perception.ron"), r"hearing_radius:\s*([\d.]+)"),
    }


def point_option(raw):
    """Reflected `Option<Vec3>`: "None", null, {"Some": v} or a bare v; v is [x, y, z] or {x, y, z}."""
    if raw is None or raw == "None":
        return None
    if isinstance(raw, dict) and "Some" in raw:
        raw = raw["Some"]
    while isinstance(raw, list) and len(raw) == 1:
        raw = raw[0]
    return list(vec3(raw))


def wanted(game):
    raw = resource_value(game, "WantedLevel")
    return {
        "heat": int(raw["heat"]),
        "stars": int(raw["stars"]),
        "last_known": point_option(raw["last_known"]),
        "seen": bool(raw["seen"]),
        "hidden": float(raw["hidden"]),
    }


def set_heat(game, heat):
    game.call("world.mutate_resources", {
        "resource": game.resource_path("WantedLevel"), "path": ".heat", "value": heat,
    })


def civilian_state(game, entity):
    for c in civilians(game):
        if c["entity"] == entity:
            return c["state"]
    return None


def mutate_civilian(game, entity, path, value):
    game.mutate_component(entity, game.component_path("Civilian"), path, value)


def poll(what, probe, timeout, interval=0.1):
    deadline = time.monotonic() + timeout
    value = None
    while time.monotonic() < deadline:
        value = probe()
        if value:
            return value
        time.sleep(interval)
    raise AssertionError(f"{what} not reached in {timeout} s (last {value!r})")


def plan_kill(people, hearing):
    """Victim with >= GROUP_MIN others within GROUP_RADIUS_M, a stand spot 5 m from it, a caller in range."""
    lo, hi = CALLER_RANGE_M
    for victim in sorted(people, key=lambda c: -sum(
            1 for o in people if o is not c and horizontal(o["position"], c["position"]) <= GROUP_RADIUS_M)):
        group = [o for o in people if o is not victim and horizontal(o["position"], victim["position"]) <= GROUP_RADIUS_M]
        if len(group) < GROUP_MIN:
            break
        vx, vy, vz = victim["position"]
        for k in range(16):
            angle = 2.0 * math.pi * k / 16
            spot = (vx + VICTIM_STAND_OFF_M * math.sin(angle), vy, vz + VICTIM_STAND_OFF_M * math.cos(angle))
            callers = [o for o in people if o is not victim
                       and lo <= math.dist(o["position"], spot) <= min(hi, hearing)]
            if callers:
                return victim, spot, callers[0], group
    raise AssertionError("GATE BROKEN: no victim with a group and a caller in range")


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    check = subprocess.run([sys.executable, str(REPO / "tools" / "fetch_assets.py"), "--check"], cwd=REPO)
    if check.returncode != 0:
        raise AssertionError("run python tools/fetch_assets.py first")
    cfg = wanted_config()
    civ = civilian_config()
    cam = camera_config()
    summary = {"seed": SEED, "wanted": cfg, "civilian": civ}
    try:
        with Game(features=("dev",), args=("--seed", str(SEED)), release=True) as game:
            # 1. Playing, golden city, chunks, a populated bubble.
            poll("GameState Playing", lambda: _playing(game), 180, 0.05)
            layout_hash = int(scalar(resource_value(game, "CityLayoutHash")))
            if layout_hash != load_golden()[SEED]:
                raise AssertionError(f"layout hash {layout_hash:#x} is not the golden hash of seed {SEED}")
            summary["chunks"] = wait_chunks(game)
            people = poll(f"{BUBBLE_MIN} civilians", lambda: (lambda p: p if len(p) >= BUBBLE_MIN else None)(
                alive(civilians(game))), BUBBLE_DEADLINE_S, 0.5)
            summary["bubble"] = len(people)
            start = wanted(game)
            if start["heat"] != 0:
                raise AssertionError(f"wanted before any crime: {start}")

            # 2. Pistol.
            me = player(game)
            gun = next(i for i in weapon_pickups(game) if i["weapon"] == "Pistol" and not i["ammo_only"])
            teleport(game, me, gun["at"])
            time.sleep(SETTLE_S)
            if player(game)["held"] != "Pistol":
                raise AssertionError(f"pistol not picked up: {player(game)['held']}")

            # 3. Victim, caller, stand spot; named mutations; one shot.
            # Teleport first: calm civilians past `recycle_distance` may be recycled at any tick, and a
            # BRP mutation of a despawned entity panics the game. Near the player they stay.
            for _ in range(PLAN_TRIES):
                victim, spot, caller, group = plan_kill(alive(civilians(game)), civ["hearing_radius"])
                me = player(game)
                teleport(game, me, [spot[0], spot[1] - me["float_height"], spot[2]])
                time.sleep(SETTLE_S)
                ids = {c["entity"] for c in alive(civilians(game))}
                if victim["entity"] in ids and caller["entity"] in ids:
                    break
            else:
                raise AssertionError(f"GATE BROKEN: the planned victim or caller vanished {PLAN_TRIES} times")
            health = game.component_path("Health")
            mutate_civilian(game, victim["entity"], ".state", HOLD)
            game.mutate_component(victim["entity"], health, ".current", 1.0)
            mutate_civilian(game, caller["entity"], ".state", HOLD)
            mutate_civilian(game, caller["entity"], ".temperament", CALLER_TEMPERAMENT)
            time.sleep(0.2)
            me = player(game)
            if horizontal(me["position"], spot) > TELEPORT_TOLERANCE_M:
                raise AssertionError(f"GATE BROKEN: stand spot blocked: {me['position']} vs {spot}")
            by_id = {c["entity"]: c for c in civilians(game)}
            victim_at = by_id[victim["entity"]]["position"]
            caller_at = by_id[caller["entity"]]["position"]
            caller_distance = math.dist(caller_at, me["position"])
            if not (civ["report_min_distance"] <= caller_distance <= civ["hearing_radius"]):
                raise AssertionError(f"GATE BROKEN: caller at {caller_distance:.1f} m from the muzzle")
            target = [victim_at[0], victim_at[1] - me["float_height"] + CHEST_M, victim_at[2]]
            miss = aim_at(game, target, cam["sensitivity_deg"])
            magazine = me["guns"][0]["magazine"]
            game.send_mouse_button("Left", CLICK_MS)
            poll("the victim's death", lambda: civilian_state(game, victim["entity"]) == "Dead", 2.0)
            shot_at = time.monotonic()
            summary["kill"] = {
                "victim": victim["entity"], "group": len(group), "caller": caller["entity"],
                "caller_distance_m": round(caller_distance, 1), "aim_miss_m": round(miss, 3),
                "magazine": (magazine, player(game)["guns"][0]["magazine"]),
            }

            # 4. The caller phones it in: Report, then done; heat and stars.
            states = []

            def call_done():
                state = civilian_state(game, caller["entity"])
                if not states or states[-1] != state:
                    states.append(state)
                return "Report" in states and state not in ("Report", None)
            poll("the caller's completed call", call_done, civ["call_seconds"] + 3.0)
            w = poll("heat", lambda: (lambda v: v if v["stars"] >= 1 else None)(wanted(game)), 1.0)
            summary["call"] = {"caller_states": states, "after_s": round(time.monotonic() - shot_at, 2),
                               "wanted": w, "hud_wanted": screenshot(game, out / "hud_wanted.png")}
            if w["heat"] < cfg["kill_person"] or w["last_known"] is None:
                raise AssertionError(f"the call did not report the kill: {w}")

            # 5. Hide far outside the circle: stars blink, then clear.
            row = cfg["stars"][w["stars"] - 1]
            hospital = resource_value(game, "HospitalSpawn")
            landmarks = resource_value(game, "CityLandmarks")
            spots = [hospital["point"], landmarks["plaza_center"], landmarks["park_center"]]
            spots = [list(vec3(s)) for s in spots]
            hide = max(spots, key=lambda s: horizontal(s, w["last_known"]))
            if horizontal(hide, w["last_known"]) <= row["search_radius"] + CIRCLE_MARGIN_M:
                raise AssertionError(f"GATE BROKEN: no landmark outside the circle: {hide}")
            me = player(game)
            teleport(game, me, hide)
            hidden_at = time.monotonic()
            time.sleep(1.0)
            first = screenshot(game, out / "hud_blinking.png")
            time.sleep(CAPTURE_GAP_S)
            second = screenshot(game, out / "hud_blinking_2.png")
            w = wanted(game)
            at = player(game)["position"]
            summary["hide"] = {"at": hide, "distance_m": round(horizontal(at, w["last_known"] or at), 1),
                               "wanted": w, "screenshots": [first, second]}
            if not (w["hidden"] > 0.0 and not w["seen"] and w["stars"] >= 1):
                raise AssertionError(f"not hiding: {w}")
            if horizontal(at, w["last_known"]) <= row["search_radius"]:
                raise AssertionError(f"GATE BROKEN: the player is inside the circle: {at}")
            time.sleep(max(0.0, hidden_at + row["clear_seconds"] - 1.0 - time.monotonic()))
            early = wanted(game)
            if early["stars"] < 1:
                raise AssertionError(f"cleared before clear_seconds - 1: {early}")
            poll("cleared wanted level", lambda: (lambda v: v if v["heat"] == 0 and v["stars"] == 0 else None)(
                wanted(game)), hidden_at + row["clear_seconds"] + 3.0 - time.monotonic())
            summary["clear"] = {"after_s": round(time.monotonic() - hidden_at, 2), "early": early,
                                "hud_clear": screenshot(game, out / "hud_clear.png")}

            # 6. BRP reachability: heat mutation -> stars.
            two = cfg["stars"][1]["heat"]
            set_heat(game, two)
            summary["mutation_two"] = poll("2 stars", lambda: (lambda v: v if v["stars"] == 2 else None)(
                wanted(game)), 2.0)
            summary["hud_two_stars"] = screenshot(game, out / "hud_two_stars.png")
            set_heat(game, 0)
            summary["mutation_zero"] = poll("0 stars", lambda: (lambda v: v if v["stars"] == 0 else None)(
                wanted(game)), 2.0)

            # 7. Clean log, clean exit.
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


def _playing(game):
    try:
        return game_state(game) == "Playing"
    except RuntimeError:
        return False


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t10")
    run(parser.parse_args().out.resolve())
