"""Runtime T12 gate: pause, "Новый город" and the minimap under a wanted level. Esc pauses (the fixed loop
stops: `AiClock` does not advance), the pause menu shows the seed (screenshot), a seed typed into the
field with `type_text` and confirmed with Enter builds that city (new `CitySeed`, golden `CityLayoutHash`,
one player, one minimap, one HUD, a full chunk grid and nothing left of the old city). With two stars the
minimap has one police dot per live cop, shown for every cop inside the view radius (screenshot with the
search circle and cones).

BRP constraint: bevy_brp_extras releases `send_keys` holds on the virtual clock, which is frozen while
paused, so each key is sent at most once per pause and the seed goes in with `type_text` (one key per
frame, no timer). The resume and the menu buttons are left to the owner run.

Hard pass/fail rests on `GameState`, `PauseMenu`, `AiClock`, `CitySeed`, `CityLayoutHash`, entity counts
and the police marker set; screenshots are evidence for the owner."""

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
from t5 import ERROR_WORDS, game_state, poll, resource_value, screenshot, state_name, wait_chunks  # noqa: E402
from t6 import rows  # noqa: E402
from t8 import scalar, variant  # noqa: E402

SEED = 1
NEW_SEED = 42
PAUSE_MENU = "State<gta_like::menu::PauseMenu>"
HEAT = 200  # two stars by wanted.ron (40, 180, ...)
RIM_TOLERANCE_M = 5.0
TYPE_WAIT_S = 0.5
MARKER_SAMPLES = 5
MARKER_MATCHES = 3


def ron_number(rel, pattern):
    match = re.search(pattern, (REPO / "assets" / rel).read_text(encoding="utf-8"), re.S)
    if not match:
        raise AssertionError(f"GATE BROKEN: {pattern!r} not found in {rel}")
    return float(match.group(1))


def chunk_grid():
    size = ron_number("world/city.ron", r"\bsize:\s*([\d.]+)")
    chunk = ron_number("world/render.ron", r"chunk_size:\s*([\d.]+)")
    n = math.ceil(size / chunk)
    return n * n


def view_radius():
    return ron_number("ui/strings.ron", r"view_radius:\s*([\d.]+)")


def state_is(game, name):
    try:
        return game_state(game) == name
    except RuntimeError:
        return False


def pause_menu(game):
    try:
        return state_name(resource_value(game, PAUSE_MENU))
    except RuntimeError:
        return None


def ai_tick(game):
    return int(scalar(resource_value(game, "AiClock")))


def count(game, component):
    path = game.component_path(component)
    return len(game.query([path], with_=[path]))


def hud_roots(game):
    return sum(1 for _, (name,) in rows(game, ["Name"]) if scalar(name) == "Hud")


def markers(game):
    return [
        {"entity": e, "target": m.get("target"), "kind": variant(m["kind"]), "visibility": variant(v)}
        for e, (m, v) in rows(game, ["MinimapMarker", "Visibility"])
    ]


def player_at(game):
    found = rows(game, ["Position"], with_="Player")
    if len(found) != 1:
        raise AssertionError(f"expected one player, found {len(found)}")
    return vec3(found[0][1][0])


def live_cops(game):
    return [
        {"entity": e, "state": variant(u["state"]), "position": vec3(p)}
        for e, (u, p) in rows(game, ["PoliceUnit", "Position"])
        if variant(u["state"]) != "Dead"
    ]


def flat(a, b):
    return math.hypot(a[0] - b[0], a[2] - b[2])


def city_counts(game):
    return {
        "player": count(game, "Player"),
        "minimap": count(game, "Minimap"),
        "hud": hud_roots(game),
    }


def marker_sample(game, radius):
    me = player_at(game)
    cops = live_cops(game)
    police = [m for m in markers(game) if m["kind"] == "Police"]
    targets = sorted(m["target"] for m in police)
    live = sorted(c["entity"] for c in cops)
    near = {c["entity"] for c in cops if flat(c["position"], me) <= radius - RIM_TOLERANCE_M}
    hidden_near = [m["target"] for m in police if m["target"] in near and m["visibility"] == "Hidden"]
    return {"targets": targets, "live": live, "near": sorted(near), "hidden_near": hidden_near,
            "ok": targets == live and not hidden_near}


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    check = subprocess.run([sys.executable, str(REPO / "tools" / "fetch_assets.py"), "--check"], cwd=REPO)
    if check.returncode != 0:
        raise AssertionError("run python tools/fetch_assets.py first")
    golden = load_golden()
    grid = chunk_grid()
    radius = view_radius()
    summary = {"seed": SEED, "new_seed": NEW_SEED, "chunk_grid": grid, "view_radius": radius}
    try:
        with Game(features=("dev",), args=("--seed", str(SEED)), release=True) as game:
            # 1. The first city with its minimap and HUD.
            poll("GameState Playing", lambda: state_is(game, "Playing"), 180, 0.05)
            layout_hash = int(scalar(resource_value(game, "CityLayoutHash")))
            if layout_hash != golden[SEED]:
                raise AssertionError(f"layout hash {layout_hash:#x} is not the golden hash of seed {SEED}")
            summary["chunks"] = wait_chunks(game)
            if summary["chunks"] != grid:
                raise AssertionError(f"{summary['chunks']} chunks, expected {grid}")
            summary["first_city"] = city_counts(game)
            if summary["first_city"] != {"player": 1, "minimap": 1, "hud": 1}:
                raise AssertionError(f"first city: {summary['first_city']}")
            kinds = [m["kind"] for m in markers(game)]
            for landmark in ("Hospital", "Station"):
                if kinds.count(landmark) != 1:
                    raise AssertionError(f"{kinds.count(landmark)} {landmark} markers")

            # 2. Esc pauses: the fixed loop stops.
            game.send_keys(["Escape"], 100)
            poll("GameState Paused", lambda: state_is(game, "Paused"), 10, 0.05)
            poll("PauseMenu Main", lambda: pause_menu(game) == "Main", 10, 0.05)
            first = ai_tick(game)
            time.sleep(1.0)
            second = ai_tick(game)
            summary["paused_ai_ticks"] = [first, second]
            if first != second:
                raise AssertionError(f"AiClock advanced while paused: {first} -> {second}")
            summary["pause_png"] = screenshot(game, out / "pause.png")

            # 3. A typed seed builds that city.
            game.type_text(str(NEW_SEED))
            time.sleep(TYPE_WAIT_S)
            game.send_keys(["Enter"], 100)
            poll("left Paused", lambda: not state_is(game, "Paused"), 10, 0.02)
            poll("GameState Playing (new city)", lambda: state_is(game, "Playing"), 180, 0.05)
            seed = int(scalar(resource_value(game, "CitySeed")))
            layout_hash = int(scalar(resource_value(game, "CityLayoutHash")))
            summary["new_city"] = {"seed": seed, "layout_hash": f"{layout_hash:#018x}"}
            if seed != NEW_SEED:
                raise AssertionError(f"CitySeed {seed}, typed {NEW_SEED}")
            if layout_hash != golden[NEW_SEED]:
                raise AssertionError(f"layout hash {layout_hash:#x} is not the golden hash of seed {NEW_SEED}")
            summary["new_city"]["chunks"] = wait_chunks(game)
            if summary["new_city"]["chunks"] != grid:
                raise AssertionError(f"new city: {summary['new_city']['chunks']} chunks, expected {grid}")
            counts = city_counts(game)
            summary["new_city"]["counts"] = counts
            if counts != {"player": 1, "minimap": 1, "hud": 1}:
                raise AssertionError(f"new city: {counts}")
            first = ai_tick(game)
            time.sleep(1.0)
            second = ai_tick(game)
            summary["new_city"]["ai_ticks"] = [first, second]
            if second <= first:
                raise AssertionError(f"AiClock does not advance in the new city: {first} -> {second}")

            # 4. Minimap under a wanted level.
            game.call("world.mutate_resources", {
                "resource": game.resource_path("WantedLevel"), "path": ".heat", "value": HEAT,
            })
            poll(
                "a cop inside the minimap radius",
                lambda: any(flat(c["position"], player_at(game)) <= radius - RIM_TOLERANCE_M
                            for c in live_cops(game)),
                120, 0.25,
            )
            samples = []
            for _ in range(MARKER_SAMPLES):
                samples.append(marker_sample(game, radius))
                time.sleep(0.3)
            summary["police_markers"] = samples
            matches = sum(1 for s in samples if s["ok"])
            if matches < MARKER_MATCHES:
                raise AssertionError(f"police markers matched the live cops in {matches}/{MARKER_SAMPLES} samples")
            summary["minimap_png"] = screenshot(game, out / "minimap_wanted.png")
            summary["frame_report"] = game.frame_report()
            summary["game_state"] = game_state(game)
            words = (*ERROR_WORDS, "shader", "wgsl")
            lines = game.log_tail(2_000_000).splitlines()
            errors = [line for line in lines
                      if "ERROR" in line and any(w.lower() in line.lower() for w in words)]
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
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t12")
    run(parser.parse_args().out)


if __name__ == "__main__":
    main()
