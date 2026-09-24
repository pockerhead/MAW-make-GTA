"""Runtime T11 gate: foot police and the arrest. A BRP heat mutation to one star brings patrol cops,
never more than the escalation row; the passive player is arrested (BUSTED screen), respawns at the
police station without guns, bat and wanted level. A second run at four stars brings SWAT (screenshot).
Named QA mutations: the player's `Loadout` gets a pistol and a bat (so the confiscation is observable)
and its armour is raised (gangs or crossfire must not kill the player before the arrest). Hard pass/fail
rests on `GameState`, `PoliceUnit`, `PoliceDispatcher`, `Loadout`, `Position` and `WantedLevel`;
screenshots are evidence for the owner. Per cop the distance to the player is logged: a cop has arrived
once it is within its kind's `keep_distance.1` (real distance, whatever its state); a cop that has not
arrived and made no progress for `STUCK_S` goes to `summary.json` `stuck` with its state (navmesh
evidence when it is `Respond`/`Search`, a fire-discipline hold when it is `Attack`). The four-star run
lasts `SWAT_WINDOW_S` so every unit has time to arrive or stall."""

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
from t5 import game_state, log_errors, resource_value, screenshot, state_name, wait_chunks  # noqa: E402
from t6 import aim_at, camera_config, player, rows, teleport  # noqa: E402
from t8 import horizontal, scalar, variant  # noqa: E402

SEED = 1
SETTLE_S = 0.5
BUSTED_PHASE = "State<gta_sim::flow::BustedPhase>"
ARMOR = 1.0e6
ARREST_DEADLINE_S = 180.0
SWAT_DEADLINE_S = 90.0
SWAT_WINDOW_S = 45.0
SWAT_AIM_M = 30.0
STUCK_S = 5.0
STUCK_PROGRESS_M = 1.0
POLL_S = 0.25
STATION_TOLERANCE_M = 1.0
CHEST_M = 1.0


def ron_text(rel):
    return (REPO / "assets" / rel).read_text(encoding="utf-8")


def ron_number(text, pattern):
    match = re.search(pattern, text, re.S)
    if not match:
        raise AssertionError(f"GATE BROKEN: {pattern!r} not found")
    return float(match.group(1))


def escalation():
    text = ron_text("police/escalation.ron")
    rows_ = re.findall(r"\(units:\s*(\d+),\s*swat:\s*(\d+),", text)
    if len(rows_) != 5:
        raise AssertionError(f"GATE BROKEN: expected 5 rows in escalation.ron, got {len(rows_)}")
    return {
        "stars": [{"units": int(u), "swat": int(s)} for u, s in rows_],
        "arrest_distance": ron_number(text, r"\bdistance:\s*([\d.]+)"),
        "arrive_m": {
            "Patrol": ron_number(text, r"patrol:\s*\(gun.*?keep_distance:\s*\([\d.]+,\s*([\d.]+)\)"),
            "Swat": ron_number(text, r"swat:\s*\(gun.*?keep_distance:\s*\([\d.]+,\s*([\d.]+)\)"),
        },
    }


def star_heats():
    heats = re.findall(r"\(heat:\s*(\d+),\s*search_radius", ron_text("wanted/wanted.ron"))
    if len(heats) != 5:
        raise AssertionError(f"GATE BROKEN: expected 5 star rows in wanted.ron, got {len(heats)}")
    return [int(h) for h in heats]


def respawn_config():
    text = ron_text("flow/respawn.ron")
    return {
        "busted_arrest": ron_number(text, r"busted_arrest:\s*([\d.]+)"),
        "busted_screen": ron_number(text, r"busted_screen:\s*([\d.]+)"),
    }


def pistol_magazine():
    return int(ron_number(ron_text("combat/weapons.ron"), r"pistol:.*?magazine:\s*(\d+)"))


def set_heat(game, heat):
    game.call("world.mutate_resources", {
        "resource": game.resource_path("WantedLevel"), "path": ".heat", "value": heat,
    })


def wanted(game):
    raw = resource_value(game, "WantedLevel")
    return {"heat": int(raw["heat"]), "stars": int(raw["stars"])}


def cops(game):
    return [
        {"entity": e, "kind": variant(u["kind"]), "state": variant(u["state"]), "position": vec3(p)}
        for e, (u, p) in rows(game, ["PoliceUnit", "Position"])
    ]


def active(units):
    return [c for c in units if c["state"] not in ("Dead", "Leave")]


def dispatcher(game):
    raw = resource_value(game, "PoliceDispatcher")
    return {"units": int(raw["units"]), "swat": int(raw["swat"])}


def busted_phase(game):
    try:
        return state_name(resource_value(game, BUSTED_PHASE))
    except RuntimeError:
        return None


def loadout(game):
    found = rows(game, ["Loadout"], with_="Player")
    return found[0][1][0]


def arm_player(game, me):
    """Named mutation: a pistol with a full magazine and a bat, so the confiscation is observable."""
    path = game.component_path("Loadout")
    game.mutate_component(me["entity"], path, ".guns[0].owned", True)
    game.mutate_component(me["entity"], path, ".guns[0].magazine", pistol_magazine())
    game.mutate_component(me["entity"], path, ".has_bat", True)
    game.mutate_component(me["entity"], game.component_path("Health"), ".armor", ARMOR)


class Tracker:
    """Per cop: flat distance to the player each poll. Arrived = within its kind's `keep_distance.1`
    (`arrive_m`), by distance only; `stuck` = the latest stall of a unit before it arrived (no
    `STUCK_PROGRESS_M` progress for `STUCK_S`). Units in `Dead`/`Leave` are not tracked."""

    def __init__(self, arrive_m):
        self.arrive_m = arrive_m
        self.best = {}
        self.first_seen = {}
        self.stuck = {}
        self.reached = {}
        self.last = {}

    def update(self, units, at, now):
        for c in active(units):
            e, d = c["entity"], horizontal(c["position"], at)
            self.first_seen.setdefault(e, now)
            self.last[e] = {"kind": c["kind"], "state": c["state"], "at_m": round(d, 1)}
            best, _ = self.best.get(e, (math.inf, now))
            if d < best - STUCK_PROGRESS_M:
                self.best[e] = (d, now)
            else:
                self.best.setdefault(e, (d, now))
            if e in self.reached:
                continue
            if d <= self.arrive_m[c["kind"]]:
                self.reached[e] = round(now - self.first_seen[e], 1)
                continue
            if now - self.best[e][1] >= STUCK_S:
                self.stuck[e] = {"state": c["state"], "at_m": round(d, 1),
                                 "since_s": round(now - self.first_seen[e], 1),
                                 "still_s": round(now - self.best[e][1], 1)}

    def report(self):
        not_arrived = {e: v for e, v in self.last.items() if e not in self.reached}
        return {"reach_s": self.reached, "not_arrived": not_arrived, "stuck": self.stuck}


def check_bounds(game, row, what):
    units = cops(game)
    live = active(units)
    swat = sum(1 for c in live if c["kind"] == "Swat")
    d = dispatcher(game)
    if len(live) > row["units"] or swat > row["swat"] or d["units"] > row["units"] or d["swat"] > row["swat"]:
        raise AssertionError(f"{what}: {len(live)} units ({swat} SWAT), dispatcher {d}, row {row}")
    return units


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    check = subprocess.run([sys.executable, str(REPO / "tools" / "fetch_assets.py"), "--check"], cwd=REPO)
    if check.returncode != 0:
        raise AssertionError("run python tools/fetch_assets.py first")
    esc = escalation()
    heats = star_heats()
    respawn = respawn_config()
    cam = camera_config()
    summary = {"seed": SEED, "escalation": esc, "star_heats": heats, "respawn": respawn}
    try:
        with Game(features=("dev",), args=("--seed", str(SEED)), release=True) as game:
            # 1. Playing, golden city, chunks; the player on the hospital sidewalk, armed by mutation.
            poll("GameState Playing", lambda: _playing(game), 180, 0.05)
            layout_hash = int(scalar(resource_value(game, "CityLayoutHash")))
            if layout_hash != load_golden()[SEED]:
                raise AssertionError(f"layout hash {layout_hash:#x} is not the golden hash of seed {SEED}")
            summary["chunks"] = wait_chunks(game)
            hospital = list(vec3(resource_value(game, "HospitalSpawn")["point"]))
            me = player(game)
            teleport(game, me, hospital)
            time.sleep(SETTLE_S)
            arm_player(game, me)
            time.sleep(SETTLE_S)
            before = loadout(game)
            if not (before["guns"][0]["owned"] and before["has_bat"]):
                raise AssertionError(f"GATE BROKEN: the arming mutation did not land: {before}")
            if wanted(game)["heat"] != 0:
                raise AssertionError(f"wanted before any crime: {wanted(game)}")

            # 2. One star: patrol cops come, the passive player is arrested.
            row = esc["stars"][0]
            set_heat(game, heats[0])
            tracker = Tracker(esc["arrive_m"])
            start = time.monotonic()
            arrest_seen = None
            while True:
                now = time.monotonic() - start
                state = game_state(game)
                if state == "Busted":
                    break
                if state != "Playing":
                    raise AssertionError(f"left Playing for {state} before the arrest")
                if now > ARREST_DEADLINE_S:
                    raise AssertionError(f"no arrest in {ARREST_DEADLINE_S} s: {tracker.report()}")
                units = check_bounds(game, row, f"1 star at {now:.1f} s")
                at = player(game)["position"]
                tracker.update(units, at, now)
                if arrest_seen is None and any(
                        c["state"] == "Arrest" and horizontal(c["position"], at) <= esc["arrest_distance"]
                        for c in units):
                    arrest_seen = round(now, 1)
                time.sleep(POLL_S)
            busted_at = time.monotonic()
            summary["arrest"] = {"cop_in_reach_s": arrest_seen, "busted_s": round(busted_at - start, 1),
                                 **tracker.report()}
            if arrest_seen is None:
                raise AssertionError("Busted without a cop in Arrest within the arrest distance")
            poll("BUSTED screen", lambda: busted_phase(game) == "Screen", respawn["busted_arrest"] + 3.0)
            time.sleep(0.3)
            summary["busted_png"] = screenshot(game, out / "busted.png")
            poll("Playing after Busted", lambda: _playing(game),
                 respawn["busted_arrest"] + respawn["busted_screen"] + 5.0)
            summary["busted_seconds"] = round(time.monotonic() - busted_at, 2)
            time.sleep(SETTLE_S)
            after = loadout(game)
            station = list(vec3(resource_value(game, "PoliceStationSpawn")["point"]))
            at = player(game)["position"]
            w = wanted(game)
            summary["respawn"] = {"loadout": after, "at": at, "station": station,
                                  "station_distance_m": round(horizontal(at, station), 2), "wanted": w}
            if any(g["owned"] for g in after["guns"]) or after["has_bat"] or after["held"] not in (None, "None"):
                raise AssertionError(f"weapons not confiscated: {after}")
            if horizontal(at, station) > STATION_TOLERANCE_M:
                raise AssertionError(f"respawned {horizontal(at, station):.2f} m from the station")
            if w["heat"] != 0:
                raise AssertionError(f"wanted kept after the arrest: {w}")

            # 3. Four stars: SWAT come; screenshot one; keep watching until SWAT_WINDOW_S (arrivals, bounds).
            me = player(game)
            game.mutate_component(me["entity"], game.component_path("Health"), ".armor", ARMOR)
            row = esc["stars"][3]
            set_heat(game, heats[3])
            tracker = Tracker(esc["arrive_m"])
            start = time.monotonic()
            swat_png = None
            while True:
                now = time.monotonic() - start
                if swat_png is not None and now > SWAT_WINDOW_S:
                    break
                if now > SWAT_DEADLINE_S:
                    raise AssertionError(f"no SWAT within {SWAT_AIM_M} m in {SWAT_DEADLINE_S} s: {tracker.report()}")
                if game_state(game) != "Playing":
                    raise AssertionError(f"left Playing at 4 stars: {game_state(game)}")
                units = check_bounds(game, row, f"4 stars at {now:.1f} s")
                me = player(game)
                tracker.update(units, me["position"], now)
                swat = [c for c in active(units) if c["kind"] == "Swat"]
                near = sorted(swat, key=lambda c: horizontal(c["position"], me["position"]))
                if swat_png is None and near and horizontal(near[0]["position"], me["position"]) <= SWAT_AIM_M:
                    target = list(near[0]["position"])
                    target[1] += CHEST_M - me["float_height"] + 0.3
                    try:
                        aim_at(game, target, cam["sensitivity_deg"])
                    except AssertionError as err:
                        summary["swat_aim"] = str(err)
                    time.sleep(0.2)
                    swat_png = screenshot(game, out / "swat.png")
                    summary["swat"] = {"after_s": round(now, 1), "distance_m": round(
                        horizontal(near[0]["position"], me["position"]), 1), "units": len(active(units)),
                        "swat": len(swat)}
                time.sleep(POLL_S)
            summary["swat_run"] = tracker.report()
            summary["swat_png"] = swat_png
            summary["frame_report"] = game.frame_report()

            # 4. Clean log, clean exit.
            set_heat(game, 0)
            summary["game_state"] = game_state(game)
            errors = log_errors(game)
            summary["log_errors"] = errors
            if errors:
                raise AssertionError("errors in the game log:\n" + "\n".join(errors))
            game.shutdown()
            game.process.wait(timeout=15)
    finally:
        (out / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False, default=str),
                                          encoding="utf-8")
    print(json.dumps(summary, indent=2, ensure_ascii=False, default=str))


def poll(what, probe, timeout, interval=0.1):
    deadline = time.monotonic() + timeout
    value = None
    while time.monotonic() < deadline:
        value = probe()
        if value:
            return value
        time.sleep(interval)
    raise AssertionError(f"{what} not reached in {timeout} s (last {value!r})")


def _playing(game):
    try:
        return game_state(game) == "Playing"
    except RuntimeError:
        return False


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t11")
    run(parser.parse_args().out.resolve())
