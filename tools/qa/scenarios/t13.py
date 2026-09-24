"""Runtime T13 gate: sound and juice. The camera carries the mirrored spatial listener, two ambience
beds play and the park bed takes over on the park range; punches, a pistol series and a wall shot
spawn shot and impact sounds within the voice caps and shake the camera; a hit taken lights the red
vignette (not with "no flashes"); a hit from a dummy on the right draws the damage arc on the right;
a wanted rise plays one stinger and pulses the stars; sirens ride on live cops and stop with the
wanted level; the pause menu clicks; death plays one sting.
Agents cannot hear: sound is checked only by its sources (`SoundStats`, `Sound` entities, their
parents, `AmbienceMix`). Named QA mutations: the player's armour is raised (NPC fire must not kill the
player before the death phase) and dropped to 0 before it; `WantedLevel.heat`; `GameSettings.no_flashes`;
a `DamageDealt` message from a dummy. Screenshots are evidence for the owner."""

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
from t5 import damage, game_state, poll, resource_value, screenshot, wait_chunks  # noqa: E402
from t6 import aim_at, camera_config, dummies, rows, teleport, weapon_pickups  # noqa: E402
from t6 import player as aim_player  # noqa: E402
from t7 import face_dummy, me  # noqa: E402
from t8 import scalar, variant  # noqa: E402
from t11 import cops, set_heat, star_heats, wanted  # noqa: E402

SEED = 1
CLASSES = ["Shot", "Impact", "Hurt", "Ui", "Stinger", "DeathSting", "Siren", "Ambience"]
ARMOR = 1.0e6
SETTLE_S = 0.5
POLL_S = 0.05
CHEST_M = 1.0
SHOT_GAP_S = 0.35
CLICK_MS = 80
SIREN_DEADLINE_S = 90.0
DAMAGE_DEALT = "gta_sim::combat::hitscan::DamageDealt"


def mix_caps():
    """Voice caps per class (index order of `CLASSES`) and ambience gains from `audio/mix.ron`."""
    text = (REPO / "assets" / "audio" / "mix.ron").read_text(encoding="utf-8")
    voices = re.search(r"voices:\s*\(shot:\s*(\d+),\s*impact:\s*(\d+),\s*hurt:\s*(\d+),\s*ui:\s*(\d+),"
                       r"\s*stinger:\s*(\d+),\s*death_sting:\s*(\d+)\)", text)
    sirens = re.search(r"max_emitters:\s*(\d+)", text)
    city = re.search(r"city_volume:\s*([\d.]+)", text)
    park = re.search(r"park_volume:\s*([\d.]+)", text)
    if not (voices and sirens and city and park):
        raise AssertionError("GATE BROKEN: audio/mix.ron layout not understood")
    caps = [int(v) for v in voices.groups()] + [int(sirens.group(1)), 2]
    return {"caps": caps, "city_volume": float(city.group(1)), "park_volume": float(park.group(1))}


def stats(game):
    raw = resource_value(game, "SoundStats")
    return {"spawned": [int(v) for v in raw["spawned"]], "peak_alive": [int(v) for v in raw["peak_alive"]]}


def spawned(game, cls):
    return stats(game)["spawned"][CLASSES.index(cls)]


def sounds(game, cls=None):
    found = [(e, variant(s["class"])) for e, (s,) in rows(game, ["Sound"])]
    return [e for e, c in found if cls is None or c == cls]


def check_peaks(game, caps, what):
    peaks = stats(game)["peak_alive"]
    over = {CLASSES[i]: (p, caps[i]) for i, p in enumerate(peaks) if p > caps[i]}
    if over:
        raise AssertionError(f"{what}: voice peaks over the cap {over}")
    return peaks


def trauma(game):
    return float(resource_value(game, "CameraShake")["trauma"])


def vignette(game):
    found = rows(game, ["Vignette"])
    if len(found) != 1:
        raise AssertionError(f"expected one Vignette (on the camera), got {len(found)}")
    return float(found[0][1][0]["intensity"])


def orbit(game):
    found = rows(game, ["OrbitCamera"])
    if len(found) != 1:
        raise AssertionError(f"expected one OrbitCamera, got {len(found)}")
    return found[0][1][0]


def panics(game):
    return "panicked at" in game.log_tail(2_000_000)


def alive(game, what):
    if panics(game):
        raise AssertionError(f"{what}: a panic in the game log:\n{game.log_tail()}")
    return game_state(game)


def set_setting(game, field, value):
    game.call("world.mutate_resources", {
        "resource": game.resource_path("GameSettings"), "path": f".{field}", "value": value,
    })


def set_armor(game, entity, value):
    game.mutate_component(entity, game.component_path("Health"), ".armor", value)


def watch(duration, probe, interval=POLL_S):
    """`probe()` every `interval` for `duration` s; the list of results."""
    end = time.monotonic() + duration
    out = []
    while time.monotonic() < end:
        out.append(probe())
        time.sleep(interval)
    return out


def arc_expected(yaw, player, attacker):
    """Python twin of `juice::damage_arc::arc_angle` (wiring check, not the gate of the formula)."""
    dx, dz = attacker[0] - player[0], attacker[2] - player[2]
    forward = (-math.sin(yaw), -math.cos(yaw))
    right = (math.cos(yaw), -math.sin(yaw))
    return math.atan2(dx * right[0] + dz * right[1], dx * forward[0] + dz * forward[1])


def star_pulse(game):
    for _, (name, pulse, transform) in rows(game, ["Name", "StarPulse", "UiTransform"], with_="StarPulse"):
        scale = transform["scale"]
        sx = scale["x"] if isinstance(scale, dict) else scale[0]
        return {"name": name, "left": float(pulse["left"]), "scale_x": float(sx)}
    raise AssertionError("no StarPulse row")


def sirens(game):
    found = rows(game, ["Sound", "ChildOf"], with_="Sound")
    return [(e, int(scalar(parent))) for e, (s, parent) in found if variant(s["class"]) == "Siren"]


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    check = subprocess.run([sys.executable, str(REPO / "tools" / "fetch_assets.py"), "--check"], cwd=REPO)
    if check.returncode != 0:
        raise AssertionError("run python tools/fetch_assets.py first")
    mix = mix_caps()
    caps = mix["caps"]
    cam = camera_config()
    heats = star_heats()
    summary = {"seed": SEED, "caps": dict(zip(CLASSES, caps)), "phases": {}, "stats": []}
    phases = summary["phases"]
    last_png = [0.0]

    def shot_png(name):
        time.sleep(max(0.0, last_png[0] + 0.15 - time.monotonic()))
        path = screenshot(game, out / name)
        last_png[0] = time.monotonic()
        return path

    try:
        with Game(features=("dev",), args=("--seed", str(SEED)), release=True) as game:
            # 1. Golden city, chunks; armour so NPC fire cannot end the run early.
            poll("GameState Playing", lambda: game_state(game) == "Playing", 180, 0.05)
            layout_hash = int(scalar(resource_value(game, "CityLayoutHash")))
            if layout_hash != load_golden()[SEED]:
                raise AssertionError(f"layout hash {layout_hash:#x} is not the golden hash of seed {SEED}")
            summary["chunks"] = wait_chunks(game)
            summary["no_audio_device"] = "No audio device" in game.log_tail(2_000_000)
            player = me(game)
            set_armor(game, player["entity"], ARMOR)
            summary["stats"].append(("start", stats(game)))

            # 2. Listener: one, on the camera, ears mirrored (rodio 0.22.2 swap).
            listeners = rows(game, ["SpatialListener", "OrbitCamera"], with_="SpatialListener")
            all_listeners = rows(game, ["SpatialListener"])
            right_x = vec3(listeners[0][1][0]["right_ear_offset"])[0] if listeners else None
            phases["listener"] = {"count": len(all_listeners), "on_camera": len(listeners), "right_ear_x": right_x}
            if len(all_listeners) != 1 or len(listeners) != 1:
                raise AssertionError(f"listener: {phases['listener']}")
            if not right_x < 0:
                raise AssertionError(f"right ear not mirrored: {phases['listener']}")

            # 3. Melee, unarmed: three punches at the middle dummy on the park range.
            targets = dummies(game)
            if len(targets) != 3:
                raise AssertionError(f"expected 3 dummies, got {len(targets)}")
            start_mix = resource_value(game, "AmbienceMix")
            player = me(game)
            if player["held"] is not None or player["melee"] != "Fists":
                raise AssertionError(f"GATE BROKEN: the player does not start unarmed: {player}")
            face_dummy(game, targets[1], cam["sensitivity_deg"])
            before = spawned(game, "Impact")
            start = time.monotonic()
            for i in range(3):
                time.sleep(max(0.0, start + i * 0.3 - time.monotonic()))
                game.send_mouse_button("Left", CLICK_MS)
            polls = watch(1.5, lambda: (spawned(game, "Impact"), trauma(game)))
            png = shot_png("melee.png")
            peaks = check_peaks(game, caps, "melee")
            phases["melee"] = {"impact_delta": polls[-1][0] - before,
                               "max_trauma": max(t for _, t in polls), "peaks": peaks, "png": png}
            if phases["melee"]["impact_delta"] < 1:
                raise AssertionError(f"punches spawned no impact: {phases['melee']}")
            if phases["melee"]["max_trauma"] <= 0:
                raise AssertionError(f"punches did not shake the camera: {phases['melee']}")
            alive(game, "melee")
            summary["stats"].append(("melee", stats(game)))

            # 4. Ambience: two beds; on the range (inside the park) the park bed has taken over.
            beds = sounds(game, "Ambience")
            park_mix = resource_value(game, "AmbienceMix")
            phases["ambience"] = {"beds": len(beds), "at_start": start_mix, "in_park": park_mix}
            if len(beds) != 2:
                raise AssertionError(f"expected 2 ambience beds, got {len(beds)}")
            if not (park_mix["park"] >= 0.9 * mix["park_volume"] and park_mix["city"] < mix["city_volume"]):
                raise AssertionError(f"park ambience did not take over: {phases['ambience']}")
            alive(game, "ambience")

            # 5. Guns: pistol, six shots at the left dummy, one shot into the ground.
            items = weapon_pickups(game)
            gun = next(i for i in items if i["weapon"] == "Pistol" and not i["ammo_only"])
            teleport(game, me(game), gun["at"])
            time.sleep(SETTLE_S)
            if me(game)["held"] != "Pistol":
                game.send_keys(["Digit2"], 100)
                time.sleep(SETTLE_S)
            if me(game)["held"] != "Pistol":
                raise AssertionError(f"pistol not picked up: {me(game)['held']}")
            target = targets[0]
            feet = list(target["position"])
            feet[1] -= me(game)["float_height"]
            teleport(game, me(game), [feet[0], feet[1], feet[2] + 5.0])
            time.sleep(1.0)
            aim_at(game, [feet[0], feet[1] + CHEST_M, feet[2]], cam["sensitivity_deg"])
            shots_before, impacts_before = spawned(game, "Shot"), spawned(game, "Impact")
            mag_before = aim_player(game)["guns"][0]["magazine"]
            start = time.monotonic()
            png = None
            for i in range(6):
                time.sleep(max(0.0, start + i * SHOT_GAP_S - time.monotonic()))
                game.send_mouse_button("Left", CLICK_MS)
                if i == 2:
                    time.sleep(0.05)
                    png = shot_png("gun_series.png")
            time.sleep(0.5)
            mag_after = aim_player(game)["guns"][0]["magazine"]
            series = {"shot_delta": spawned(game, "Shot") - shots_before,
                      "impact_delta": spawned(game, "Impact") - impacts_before,
                      "magazine": (mag_before, mag_after), "png": png}
            if mag_before - mag_after != 6:
                raise AssertionError(f"GATE BROKEN: six clicks did not fire six shots: {series}")
            if series["shot_delta"] < 6 or series["impact_delta"] < 1:
                raise AssertionError(f"gun series: {series}")
            here = me(game)
            ground = [here["position"][0], here["position"][1] - here["float_height"], here["position"][2] - 4.0]
            aim_at(game, ground, cam["sensitivity_deg"])
            time.sleep(0.4)
            before = spawned(game, "Impact")
            game.send_mouse_button("Left", CLICK_MS)
            time.sleep(0.4)
            series["ground_impact_delta"] = spawned(game, "Impact") - before
            series["peaks"] = check_peaks(game, caps, "guns")
            phases["guns"] = series
            if series["ground_impact_delta"] < 1:
                raise AssertionError(f"a shot into the ground spawned no impact: {series}")
            alive(game, "guns")
            summary["stats"].append(("guns", stats(game)))

            # 6. Hurt: vignette, shake, thud; "no flashes" hides the vignette.
            before = spawned(game, "Hurt")
            damage(game, 10)
            polls = watch(0.1, lambda: (vignette(game), trauma(game)))
            png = shot_png("hurt.png")
            polls += watch(0.2, lambda: (vignette(game), trauma(game)))
            hurt = {"max_vignette": max(v for v, _ in polls), "max_trauma": max(t for _, t in polls),
                    "hurt_delta": spawned(game, "Hurt") - before, "png": png}
            if hurt["max_vignette"] <= 0 or hurt["max_trauma"] <= 0 or hurt["hurt_delta"] < 1:
                raise AssertionError(f"hurt feedback: {hurt}")
            time.sleep(1.0)
            set_setting(game, "no_flashes", True)
            time.sleep(0.2)
            damage(game, 10)
            polls = watch(0.3, lambda: vignette(game))
            hurt["no_flashes_max_vignette"] = max(polls)
            set_setting(game, "no_flashes", False)
            phases["hurt"] = hurt
            if hurt["no_flashes_max_vignette"] > 1e-4:
                raise AssertionError(f"vignette shown with no flashes: {hurt}")
            alive(game, "hurt")

            # 7. Damage arc: a hit from the rightmost dummy while facing the middle one.
            middle, right = targets[1], targets[2]
            feet = list(middle["position"])
            feet[1] -= me(game)["float_height"]
            teleport(game, me(game), [feet[0], feet[1], feet[2] + 5.0])
            time.sleep(1.0)
            aim_at(game, [feet[0], feet[1] + CHEST_M, feet[2]], cam["sensitivity_deg"])
            here = me(game)
            game.call("world.write_message", {"message": DAMAGE_DEALT, "value": {
                "shooter": right["entity"], "shot": 0, "target": here["entity"], "point": list(here["position"]),
                "damage": 0, "headshot": False, "killed": False,
            }})
            arcs = poll("DamageArc", lambda: rows(game, ["DamageArc"]), 0.2, 0.02)
            arc = arcs[0][1][0]
            yaw = float(orbit(game)["yaw"])
            expected = arc_expected(yaw, me(game)["position"], right["position"])
            png = shot_png("damage_arc.png")
            phases["arc"] = {"angle": arc["angle"], "expected": expected, "yaw": yaw, "png": png}
            if abs(arc["angle"] - expected) > 0.15 or arc["angle"] <= 0:
                raise AssertionError(f"damage arc: {phases['arc']}")
            alive(game, "arc")

            # 8. Wanted: one stinger and a star pulse on a rise from 0 to two stars.
            set_armor(game, me(game)["entity"], ARMOR)
            set_heat(game, 0)
            poll("wanted cleared", lambda: wanted(game)["stars"] == 0, 5.0)
            time.sleep(0.3)
            before = spawned(game, "Stinger")
            set_heat(game, heats[1])
            pulses = watch(0.4, lambda: star_pulse(game), 0.02)
            time.sleep(0.2)
            stingers = spawned(game, "Stinger") - before
            live_pulse = [p for p in pulses if p["left"] > 0 and abs(p["scale_x"] - 1.0) > 1e-3]
            png = shot_png("wanted.png")
            phases["wanted"] = {"stinger_delta": stingers, "pulses": pulses[:6], "live_pulse": len(live_pulse),
                                "wanted": wanted(game), "png": png}
            if stingers != 1:
                raise AssertionError(f"expected one wanted stinger: {phases['wanted']}")
            if not live_pulse:
                raise AssertionError(f"the stars did not pulse: {phases['wanted']}")
            alive(game, "wanted")

            # 9. Sirens on live cops, gone with the wanted level.
            start = time.monotonic()
            carried = poll("a siren", lambda: sirens(game), SIREN_DEADLINE_S, 0.25)
            units = {c["entity"]: c for c in cops(game)}
            here = me(game)["position"]
            siren_rows = [{"parent": p, "state": units.get(p, {}).get("state"),
                           "distance_m": round(math.dist(units[p]["position"], here), 1) if p in units else None}
                          for _, p in carried]
            bad = [r for r in siren_rows if r["state"] in (None, "Dead", "Leave")]
            if bad:
                # A cop can die or leave between two re-picks (1 s): look again after one.
                time.sleep(1.2)
                units = {c["entity"]: c for c in cops(game)}
                carried = sirens(game)
                bad = [p for _, p in carried if units.get(p, {}).get("state") in (None, "Dead", "Leave")]
            phases["sirens"] = {"after_s": round(time.monotonic() - start, 1), "count": len(carried),
                                "rows": siren_rows, "bad_after_repick": bad}
            if not 1 <= len(carried) <= caps[CLASSES.index("Siren")] or bad:
                raise AssertionError(f"sirens: {phases['sirens']}")
            set_heat(game, 0)
            poll("sirens gone", lambda: not sirens(game), 3.0, 0.1)
            check_peaks(game, caps, "sirens")
            alive(game, "sirens")
            summary["stats"].append(("sirens", stats(game)))

            # 10. Death: one sting on "ПОТРАЧЕНО", then back to Playing.
            set_armor(game, me(game)["entity"], 0.0)
            time.sleep(0.2)
            before = spawned(game, "DeathSting")
            damage(game, 10_000)
            poll("Wasted", lambda: game_state(game) == "Wasted", 1.0, 0.02)
            time.sleep(1.0)
            phases["death"] = {"stinger_delta": spawned(game, "DeathSting") - before}
            if phases["death"]["stinger_delta"] != 1:
                raise AssertionError(f"death sting: {phases['death']}")
            poll("Playing after Wasted", lambda: game_state(game) == "Playing", 15.0, 0.1)
            alive(game, "death")

            # 11. UI: Escape pauses with the pause sound. Last: the Escape hold cannot release while
            # `Time<Virtual>` is paused, so the run ends in the pause menu.
            summary["stats"].append(("before pause", stats(game)))
            summary["peaks"] = dict(zip(CLASSES, check_peaks(game, caps, "end")))
            summary["frame_report"] = game.frame_report()
            before = spawned(game, "Ui")
            game.send_keys(["Escape"], 100)
            poll("GameState Paused", lambda: game_state(game) == "Paused", 5)
            ui_delta = poll("pause sound", lambda: spawned(game, "Ui") - before, 0.5, 0.02)
            png = shot_png("pause.png")
            phases["ui"] = {"ui_delta": ui_delta, "png": png}
            alive(game, "ui")

            # 12. Clean exit.
            game.shutdown()
            game.process.wait(timeout=15)
    finally:
        (out / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False, default=str),
                                          encoding="utf-8")
    print(json.dumps(summary, indent=2, ensure_ascii=False, default=str))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t13")
    run(parser.parse_args().out.resolve())
