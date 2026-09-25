"""Runtime T16 §1 evidence: exercises the GDD §1 points no earlier scenario touches (TASK-017 PLAN §1.4
gaps), as evidence for the TASK-031 agent playtest. Hard assertions rest on components only; the
screenshots are for the reader.

Session 1, no `--seed` (P1): the main menu, Enter with an empty seed field (clock seed), the loading
screen, a city that is not seed 1, exactly one player.
Session 2, `--seed 1` (P8 first, at the spawn in the traffic of the centre):
- P2 camera vs wall: the player 1 m from the city edge wall (t14 `WALL_FACE_Z`), the boom turned into
  the wall is pulled in (`OrbitCamera.distance` < camera.ron distance - 0.5, camera on the player's
  side); turned away, it is back at full length.
- P8 junction yield: `TrafficCar` rows every 0.25 s for 15 s; a car queued at a junction
  (`waiting`, standing) later moves on to another segment.
- P5 gang Warn -> P3 drop pickup: 6 m from the gang-0 HQ group with no gang heat, a member warns
  within `warn_seconds` + 1 s; named QA mutation `Health.current = 0` on one member (no shot, gang heat
  reported); its dropped gun is picked up (owned, or its reserve grew).
- P9 death keeps guns: the range pistol, 2 stars, one full-HUD screenshot, lethal `DebugDamage`,
  Wasted -> Playing at the hospital with the pistol, magazine and reserve unchanged.
- P7 run over: the parked car nearest the range dummies 15 m in front of one, facing it, F at its door,
  W for 2 s: the dummy loses health or is knocked down within 3 s.
- P10 settings through the UI (Windows only, real OS input: BRP holds never release while paused):
  Esc, "Настройки", the "Инверсия Y" toggle flips `GameSettings.invert_y` (and back).
Each phase records its own section; a failed phase does not hide the others, the run fails at the end.
BRP constraint: every key is held once per window, an entity is re-queried right before it is mutated.
"""

import argparse
import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import traceback

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from brp import Game, REPO, load_golden, vec3  # noqa: E402
from t5 import damage, game_state, poll, resource_value, screenshot, wait_chunks  # noqa: E402
from t6 import dummies, player, rows, teleport, weapon_name  # noqa: E402
from t7 import reaction_name  # noqa: E402
from t8 import scalar, variant  # noqa: E402
from t9 import gang_posts, heat, horizontal, members, pistol_pickups, player_territory, spawn_ring  # noqa: E402
from t11 import set_heat, star_heats, wanted  # noqa: E402
from t12 import pause_menu  # noqa: E402
from t14 import WALL_FACE_Z, car, cars, driving, put, ron_number, ron_tuple, rotate  # noqa: E402

SEED = 1
TITLE = "QA-T16-S1"
SETTLE_S = 0.5
WALL_GAP_M = 1.0
PULLED_IN_M = 0.5
FULL_LENGTH_TOLERANCE_M = 0.2
YIELD_WATCH_S = 15.0
YIELD_SAMPLE_S = 0.25
STANDING_MPS = 0.5
MOVING_MPS = 1.0
WARN_STAND_M = 6.0
DROP_NEAR_M = 3.0
DROP_WAIT_S = 3.0
RUN_UP_M = 15.0
RUN_OVER_WAIT_S = 3.0
DRIVE_MS = 2000
HOSPITAL_M = 1.0


def phase(summary, name, body):
    """Runs one phase; records its result or the error instead of stopping the run."""
    started = time.monotonic()
    section = summary.setdefault(name, {})
    try:
        body(section)
        section["ok"] = True
    except Exception as error:  # a failed phase is evidence too; the run fails at the end
        section["ok"] = False
        section["error"] = f"{type(error).__name__}: {error}"
        section["trace"] = traceback.format_exc(limit=3)
    section["seconds"] = round(time.monotonic() - started, 1)


def wait_playing(game, timeout=180):
    deadline = time.monotonic() + timeout
    while True:
        try:
            if game_state(game) == "Playing":
                return
        except RuntimeError:
            pass
        if time.monotonic() > deadline:
            raise AssertionError("GameState never became Playing")
        time.sleep(0.05)


def players(game):
    return len(rows(game, ["Position"], with_="Player"))


# P1 -------------------------------------------------------------------------------------------------

def p1_menu(out, section):
    with Game(features=("dev",), args=(), release=True) as game:
        poll("GameState MainMenu", lambda: game_state(game) == "MainMenu", 180, 0.1)
        time.sleep(1.0)
        section["menu_png"] = screenshot(game, out / "p1_menu.png")
        game.send_keys(["Enter"], 100)
        seen = []
        deadline = time.monotonic() + 180
        while time.monotonic() < deadline:
            state = game_state(game)
            if not seen or seen[-1] != state:
                seen.append(state)
                if state == "Loading":
                    try:
                        section["loading_png"] = screenshot(game, out / "p1_loading.png")
                    except AssertionError as error:
                        section["loading_png"] = f"not caught: {error}"
            if state == "Playing":
                break
            time.sleep(0.05)
        section["states"] = seen
        if seen[-1] != "Playing":
            raise AssertionError(f"never Playing after Enter: {seen}")
        wait_chunks(game)
        seed = int(scalar(resource_value(game, "CitySeed")))
        layout = int(scalar(resource_value(game, "CityLayoutHash")))
        section.update(seed=seed, layout_hash=hex(layout), players=players(game))
        section["city_png"] = screenshot(game, out / "p1_city.png")
        if seed == SEED and layout == load_golden()[SEED]:
            raise AssertionError("the menu start gave the seed-1 city: the clock seed was not used")
        if section["players"] != 1:
            raise AssertionError(f"{section['players']} players after the start")
        game.shutdown()
        game.process.wait(timeout=15)


# P2 -------------------------------------------------------------------------------------------------

def orbit(game):
    found = rows(game, ["OrbitCamera", "Transform"])
    if len(found) != 1:
        raise AssertionError(f"expected one OrbitCamera, got {len(found)}")
    entity, (cam, transform) = found[0]
    return entity, cam, vec3(transform["translation"])


def p2_camera_wall(game, out, section):
    boom = ron_number("camera/camera.ron", "distance")
    me = player(game)
    feet = [0.0, me["position"][1] - me["float_height"], WALL_FACE_Z - WALL_GAP_M]
    teleport(game, me, feet)
    time.sleep(SETTLE_S)
    entity, _, _ = orbit(game)
    path = game.component_path("OrbitCamera")
    # yaw 0 looks along -Z: the camera sits on the +Z side of the player, towards the wall.
    game.mutate_component(entity, path, ".yaw", 0.0)
    time.sleep(SETTLE_S)
    _, cam, at = orbit(game)
    section["into_wall"] = {"distance": cam["distance"], "camera": at, "player": player(game)["position"]}
    section["into_wall_png"] = screenshot(game, out / "p2_into_wall.png")
    if cam["distance"] >= boom - PULLED_IN_M:
        raise AssertionError(f"boom into the wall {cam['distance']:.2f} m >= {boom} - {PULLED_IN_M}")
    if at[2] >= WALL_FACE_Z:
        raise AssertionError(f"camera z {at[2]:.2f} behind the wall face {WALL_FACE_Z}")
    game.mutate_component(entity, path, ".yaw", math.pi)
    time.sleep(3 * SETTLE_S)
    _, cam, at = orbit(game)
    section["away"] = {"distance": cam["distance"], "camera": at}
    section["away_png"] = screenshot(game, out / "p2_away.png")
    if cam["distance"] < boom - FULL_LENGTH_TOLERANCE_M:
        raise AssertionError(f"boom away from the wall {cam['distance']:.2f} m < {boom} m")


# P8 -------------------------------------------------------------------------------------------------

def p8_junction_yield(game, out, section):
    history = {}
    t0 = time.monotonic()
    while time.monotonic() - t0 < YIELD_WATCH_S:
        t = time.monotonic() - t0
        for e, (car_row,) in rows(game, ["TrafficCar"]):
            history.setdefault(e, []).append({
                "t": round(t, 2),
                "segment": json.dumps(car_row["segment"], sort_keys=True),
                "speed": float(car_row["speed"]),
                "waiting": car_row["waiting"] not in (None, "None"),
            })
        time.sleep(YIELD_SAMPLE_S)
    yielded = []
    for e, samples in history.items():
        for k, s in enumerate(samples):
            if not (s["waiting"] and s["speed"] < STANDING_MPS):
                continue
            later = [x for x in samples[k + 1:] if x["segment"] != s["segment"] and x["speed"] > MOVING_MPS]
            if later:
                yielded.append({"entity": e, "waited_at": s["t"], "moved_on_at": later[0]["t"]})
                break
    section["cars_seen"] = len(history)
    section["yielded"] = yielded
    section["png"] = screenshot(game, out / "p8_traffic.png")
    if not yielded:
        raise AssertionError(f"no car of {len(history)} waited at a junction and then moved on in {YIELD_WATCH_S} s")


# P5 + P3 --------------------------------------------------------------------------------------------

def dropped_pickups(game):
    dropped = {row["entity"] for row in game.query([game.component_path("Dropped")])}
    return [
        {"entity": e, "weapon": p["weapon"], "ammo_only": p["ammo_only"], "at": vec3(t["translation"])}
        for e, (p, t) in rows(game, ["WeaponPickup", "Transform"])
        if e in dropped
    ]


def flat(a, b):
    return math.hypot(a[0] - b[0], a[2] - b[2])


def p5_warn_and_drop(game, out, section):
    gangs = (REPO / "assets" / "gang" / "gangs.ron").read_text(encoding="utf-8")
    warn_seconds = float(re.search(r"warn_seconds:\s*([\d.]+)", gangs).group(1))
    warn_distance = float(re.search(r"warn_distance:\s*([\d.]+)", gangs).group(1))
    if WARN_STAND_M >= warn_distance:
        raise AssertionError(f"GATE BROKEN: stand-off {WARN_STAND_M} m >= warn_distance {warn_distance} m")
    # As t9: the group spawns while the player stands on a post in the spawn ring of the HQ.
    posts = gang_posts(game, 0)
    hq = posts[0]
    inner, outer = spawn_ring()
    ring = [p for p in posts[1:] if inner <= horizontal(p, hq) <= outer]
    if not ring:
        raise AssertionError("GATE BROKEN: no gang-0 post in the spawn ring of the HQ")
    teleport(game, player(game), min(ring, key=lambda p: horizontal(p, hq)))
    deadline = time.monotonic() + 15
    while True:
        group = [m for m in members(game) if m["gang"] == 0 and m["post"] == 0 and m["state"] != "Dead"]
        if group:
            break
        if time.monotonic() > deadline:
            raise AssertionError("no gang-0 HQ group within 15 s")
        time.sleep(0.1)
    heat0 = heat(game)[0]
    section["heat_before"] = heat0
    if heat0 > 0.0:
        raise AssertionError(f"GATE BROKEN: gang 0 is already heated ({heat0} s)")
    cx = sum(m["position"][0] for m in group) / len(group)
    cz = sum(m["position"][2] for m in group) / len(group)
    away = [hq[0] - cx, 0.0, hq[2] - cz]
    length = math.hypot(away[0], away[2])
    unit = [away[0] / length, 0.0, away[2] / length] if length > 0.5 else [1.0, 0.0, 0.0]
    teleport(game, player(game), [cx + unit[0] * WARN_STAND_M, hq[1], cz + unit[2] * WARN_STAND_M])
    time.sleep(SETTLE_S)
    section["player_territory"] = player_territory(game)
    ids = {m["entity"] for m in group}
    poll("a member in Warn", lambda: any(m["state"] == "Warn" for m in members(game) if m["entity"] in ids),
         warn_seconds + 1.0 + 2 * SETTLE_S, 0.1)
    section["states"] = {str(m["entity"]): m["state"] for m in members(game) if m["entity"] in ids}
    section["warn_png"] = screenshot(game, out / "p5_warn.png")

    # P3: a gun dropped by a killed member.
    victim = next(m for m in members(game) if m["entity"] in ids and m["state"] != "Dead")
    gun = weapon_name(next(l for e, (l,) in rows(game, ["Loadout"], with_="GangMember")
                           if e == victim["entity"])["held"]) or "?"
    game.mutate_component(victim["entity"], game.component_path("Health"), ".current", 0.0)
    drop = poll("a dropped gun next to the killed member",
                lambda: [d for d in dropped_pickups(game) if flat(d["at"], victim["position"]) <= DROP_NEAR_M],
                DROP_WAIT_S, 0.1)[0]
    section["victim"] = {"entity": victim["entity"], "held": gun}
    section["drop"] = drop
    section["heat_after_kill"] = heat(game)[0]
    if section["heat_after_kill"] > heat0:
        section["heat_note"] = "gang heat rose after the QA kill (reported, not asserted)"
    index = ["Pistol", "Smg", "Shotgun"].index(drop["weapon"])
    before = player(game)["guns"][index]
    teleport(game, player(game), drop["at"])
    time.sleep(SETTLE_S)
    after = player(game)["guns"][index]
    section["slot"] = {"before": before, "after": after}
    section["pickup_png"] = screenshot(game, out / "p3_pickup.png")
    if not ((after["owned"] and not before["owned"]) or after["reserve"] > before["reserve"]):
        raise AssertionError(f"the dropped {drop['weapon']} was not picked up: {before} -> {after}")


# P9 -------------------------------------------------------------------------------------------------

def p9_death_keeps_guns(game, out, section):
    guns = pistol_pickups(game)
    if not guns:
        raise AssertionError("no pistol pickup on the range")
    teleport(game, player(game), guns[0])
    time.sleep(SETTLE_S)
    before = player(game)
    pistol = before["guns"][0]
    if not pistol["owned"]:
        raise AssertionError(f"GATE BROKEN: the range pistol was not picked up: {pistol}")
    set_heat(game, star_heats()[1])
    poll("2 stars", lambda: wanted(game)["stars"] >= 2, 3.0, 0.1)
    section["hud_png"] = screenshot(game, out / "p9_hud.png")
    hospital = vec3(resource_value(game, "HospitalSpawn")["point"])
    damage(game, 10000.0)
    poll("GameState Wasted", lambda: game_state(game) == "Wasted", 3.0)
    poll("GameState Playing", lambda: game_state(game) == "Playing", 15.0)
    time.sleep(SETTLE_S)
    after = player(game)
    kept = after["guns"][0]
    section.update(before=pistol, after=kept, hospital=hospital, at=after["position"])
    section["respawn_png"] = screenshot(game, out / "p9_respawn.png")
    if not kept["owned"] or (kept["magazine"], kept["reserve"]) != (pistol["magazine"], pistol["reserve"]):
        raise AssertionError(f"the pistol did not survive death unchanged: {pistol} -> {kept}")
    distance = flat(after["position"], hospital)
    if distance >= HOSPITAL_M:
        raise AssertionError(f"respawned {distance:.2f} m from the hospital point")


# P7 -------------------------------------------------------------------------------------------------

def dummy_state(game, entity):
    health = next((d["health"] for d in dummies(game) if d["entity"] == entity), None)
    reaction = next((reaction_name(v) for e, (_, v) in rows(game, ["Dummy", "HitReaction"]) if e == entity), None)
    return health, reaction


def p7_run_over(game, out, section):
    door = ron_tuple("vehicle/sedan.ron", "door")
    targets = dummies(game)
    if not targets:
        raise AssertionError("no range dummies")
    target = targets[len(targets) // 2]
    parked = cars(game)
    if not parked:
        raise AssertionError("no parked car")
    chosen = min(parked, key=lambda c: flat(c["position"], target["position"]))
    paths = (game.component_path("Position"), game.component_path("Transform"), game.component_path("Rotation"))
    # The range runs from the park centre (+Z) to the dummy row: the car stands on it facing -Z.
    at = (target["position"][0], chosen["position"][1] + 0.3, target["position"][2] + RUN_UP_M)
    identity = (0.0, 0.0, 0.0, 1.0)
    put(game, chosen["entity"], paths, at, identity)
    game.mutate_component(chosen["entity"], game.component_path("LinearVelocity"), "", [0.0, 0.0, 0.0])
    game.mutate_component(chosen["entity"], game.component_path("AngularVelocity"), "", [0.0, 0.0, 0.0])
    time.sleep(1.0)
    parked_at = car(game, chosen["entity"])["position"]
    offset = rotate(identity, door)
    me = player(game)
    teleport(game, me, [parked_at[0] + offset[0], target["position"][1] - me["float_height"] + 0.1,
                        parked_at[2] + offset[2]])
    time.sleep(0.3)
    game.send_keys(["KeyF"], 100)
    poll("Driving", lambda: driving(game) == chosen["entity"], 2.0, 0.05)
    health0, _ = dummy_state(game, target["entity"])
    game.send_keys(["KeyW"], DRIVE_MS)
    t0 = time.monotonic()
    lowest, reactions = health0, set()
    while time.monotonic() - t0 < RUN_OVER_WAIT_S:
        health, reaction = dummy_state(game, target["entity"])
        if health is not None:
            lowest = min(lowest, health)
        if reaction:
            reactions.add(reaction)
        time.sleep(0.05)
    section.update(dummy=target["entity"], car=chosen["entity"], health_before=health0, lowest=lowest,
                   reactions=sorted(reactions))
    section["png"] = screenshot(game, out / "p7_run_over.png")
    if not (lowest < health0 or "KnockedDown" in reactions):
        raise AssertionError(f"the dummy was not hit: health {health0} -> {lowest}, reactions {sorted(reactions)}")


# P10 ------------------------------------------------------------------------------------------------

def texts(game):
    tp, cp = game.component_path("Text"), game.component_path("ChildOf")
    out = {}
    for r in game.query([tp, cp], with_=[tp]):
        t = r["components"][tp]
        parent = r["components"][cp]
        out[r["entity"]] = (t[0] if isinstance(t, list) else t, int(parent[0] if isinstance(parent, list) else parent))
    return out


def parent_of(game, entity):
    cp = game.component_path("ChildOf")
    value = game.call("world.get_components", {"entity": entity, "components": [cp]})["components"][cp]
    return int(value[0] if isinstance(value, list) else value)


def ui_center(game, entity):
    gt = game.component_path("UiGlobalTransform")
    g = game.call("world.get_components", {"entity": entity, "components": [gt]})["components"][gt]
    if isinstance(g, list) and len(g) == 6 and all(isinstance(v, (int, float)) for v in g):
        return g[4], g[5]
    tr = g.get("translation") if isinstance(g, dict) else g[0]["translation"]
    return (tr[0], tr[1]) if isinstance(tr, list) else (tr["x"], tr["y"])


def invert_y(game):
    return bool(resource_value(game, "GameSettings")["invert_y"])


def p10_dump(game, out, section, why):
    """State at a P10 failure, so the next flake explains itself (no retry)."""
    section["failure_state"] = {
        "why": why,
        "game_state": game_state(game),
        "pause_menu": pause_menu(game),
        "texts": [s for s, _ in texts(game).values()],
        "screenshot": screenshot(game, out / "p10_failure.png"),
    }


def p10_settings(game, out, section):
    if os.name != "nt":
        section["skipped"] = "real OS clicks need Windows (tools/qa/osinput.py)"
        return
    import osinput

    hwnd = osinput.window(TITLE)
    # Whether the game window held the foreground before each OS input step (failure diagnosis).
    section["foreground"] = fg = []

    def step(label):
        held = osinput.user32.GetForegroundWindow() == hwnd
        fg.append([label, held])
        if not held:
            p10_dump(game, out, section, f"game window lost the foreground before {label}")
            raise AssertionError(f"OS input: the game window is not in the foreground before {label}")
    # A real Esc, not a BRP hold: BRP releases on the virtual clock, which the pause freezes, and a
    # key still held swallows the next press.
    osinput.focus(hwnd)
    step("esc")
    osinput.key("Escape")
    try:
        poll("GameState Paused", lambda: game_state(game) == "Paused", 5.0, 0.05)
    except AssertionError:
        p10_dump(game, out, section, "Esc not taken")
        raise
    time.sleep(SETTLE_S)
    osinput.focus(hwnd)
    # Re-queried after the focus step, right before the click: an entity read earlier may be gone.
    settings = [p for (s, p) in texts(game).values() if "астрой" in s]
    if not settings:
        p10_dump(game, out, section, "no settings button")
        raise AssertionError("no settings button on the pause menu")
    step("settings")
    osinput.click(hwnd, *ui_center(game, settings[0]))
    time.sleep(SETTLE_S)
    labels = texts(game)
    rows_ = [p for (s, p) in labels.values() if "Инверсия" in s]
    if not rows_:
        p10_dump(game, out, section, "settings click not taken")
        raise AssertionError(f"the settings screen did not open: {[s for s, _ in labels.values()]}")
    row = rows_[0]
    toggles = [p for (s, p) in labels.values() if s == "↔" and parent_of(game, p) == row]
    if len(toggles) != 1:
        raise AssertionError(f"GATE BROKEN: expected one toggle in the invert-Y row, got {toggles}")
    section["settings_png"] = screenshot(game, out / "p10_settings.png")
    before = invert_y(game)
    osinput.focus(hwnd)
    step("toggle1")
    osinput.click(hwnd, *ui_center(game, toggles[0]))
    flipped = invert_y(game)
    time.sleep(SETTLE_S)
    step("toggle2")
    # The row may be rebuilt after a change: find the toggle again right before the click.
    row = [p for (s, p) in texts(game).values() if "Инверсия" in s][0]
    toggle = [p for (s, p) in texts(game).values() if s == "↔" and parent_of(game, p) == row][0]
    osinput.click(hwnd, *ui_center(game, toggle))
    restored = invert_y(game)
    section.update(before=before, after_click=flipped, after_second_click=restored)
    if flipped == before or restored != before:
        p10_dump(game, out, section, "toggle click not taken")
    osinput.key("Escape")
    osinput.key("Escape")
    if flipped == before or restored != before:
        raise AssertionError(f"GameSettings.invert_y {before} -> {flipped} -> {restored}")


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    check = subprocess.run([sys.executable, str(REPO / "tools" / "fetch_assets.py"), "--check"], cwd=REPO)
    if check.returncode != 0:
        raise AssertionError("run python tools/fetch_assets.py first")
    summary = {}
    try:
        phase(summary, "P1_menu", lambda s: p1_menu(out, s))
        with Game(features=("dev",), args=("--seed", str(SEED), "--window-title", TITLE), release=True) as game:
            wait_playing(game)
            summary["chunks"] = wait_chunks(game)
            for name, body in [
                # At the spawn, in the traffic of the centre; the wall of P2 is at the city edge.
                ("P8_junction_yield", p8_junction_yield),
                ("P2_camera_wall", p2_camera_wall),
                ("P5_P3_warn_and_drop", p5_warn_and_drop),
                ("P9_death_keeps_guns", p9_death_keeps_guns),
                ("P7_run_over", p7_run_over),
                ("P10_settings", p10_settings),
            ]:
                phase(summary, name, lambda s, body=body: body(game, out, s))
            errors = [line for line in game.log_tail(2_000_000).splitlines() if "ERROR" in line]
            summary["log_errors"] = errors
            game.shutdown()
            game.process.wait(timeout=15)
    finally:
        (out / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(summary, indent=2, ensure_ascii=True))
    failed = [name for name, s in summary.items() if isinstance(s, dict) and s.get("ok") is False]
    if failed or summary.get("log_errors"):
        raise AssertionError(f"failed phases {failed}; log errors {len(summary.get('log_errors', []))}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t16_s1")
    run(parser.parse_args().out.resolve())
