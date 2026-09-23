"""Runtime T4 gate: the Kenney humanoid loads with a wired animation player, and `AnimState` follows
W / Shift+W / Alt+W / Space in the live game. Screenshots every 200 ms are evidence for the owner."""

import argparse
import json
import math
from pathlib import Path
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from brp import Game, REPO, vec3  # noqa: E402

PNG = b"\x89PNG\r\n\x1a\n"
SEED = 1
PLAYER_LIFT = 1.2
ERROR_WORDS = ("gltf", "asset", "Failed to load", "animation")
POLL_S = 0.05
SHOT_EVERY_MS = 200
# name, keys, hold ms, observe ms, expected state, body speed m/s (walk/run/sprint from locomotion.ron)
GAIT_PHASES = (
    ("run", ["KeyW"], 1200, 1500, "Run", 4.5),
    ("sprint", ["KeyW", "ShiftLeft"], 1200, 1500, "Sprint", 6.8),
    ("walk", ["KeyW", "AltLeft"], 1200, 1500, "Walk", 1.8),
)
GAIT_WINDOW_MS = (400, 1100)
JUMP_PHASE = ("jump", ["Space"], 150, 2000)
JUMP_REST_MS = 300


def resource_value(game, suffix):
    value = game.call("world.get_resources", {"resource": game.resource_path(suffix)})["value"]
    if isinstance(value, list) and len(value) == 1:
        value = value[0]
    return value


def player_position(game):
    player = game.component_path("Player")
    position = game.component_path("Position")
    found = game.query([position], with_=[player])
    if len(found) != 1:
        raise AssertionError(f"expected one Player row, got {len(found)}")
    return found[0]["entity"], position, vec3(found[0]["components"][position])


def teleport(game, target):
    entity, position, _ = player_position(game)
    game.mutate_component(entity, position, "", list(target))
    time.sleep(2.0)
    return player_position(game)[2]


def face(game, outward, pitch_deg):
    """Point the orbit camera along world (x, z) `outward`: forward = R_y(yaw) * -Z."""
    orbit = game.component_path("OrbitCamera")
    rows = game.query([orbit], with_=[orbit])
    if len(rows) != 1:
        raise AssertionError(f"expected one OrbitCamera row, got {len(rows)}")
    entity = rows[0]["entity"]
    game.mutate_component(entity, orbit, ".yaw", math.atan2(-outward[0], -outward[1]))
    game.mutate_component(entity, orbit, ".pitch", math.radians(pitch_deg))
    time.sleep(0.5)


def log_errors(game):
    lines = game.log_tail(2_000_000).splitlines()
    return [
        line for line in lines
        if "ERROR" in line and any(word.lower() in line.lower() for word in ERROR_WORDS)
    ]


def rows(game, components, with_):
    return game.query(components, with_=with_)


def wait_model(game, timeout=60):
    """Liveness of on_model_ready on the real GLB: one model, and a player wired with graph and transitions."""
    model = game.component_path("CharacterModel")
    wired = [game.component_path(name) for name in ("AnimationPlayer", "AnimationGraphHandle", "AnimationTransitions")]
    deadline = time.monotonic() + timeout
    counts = (0, 0)
    while time.monotonic() < deadline:
        counts = (len(rows(game, [model], [model])), len(rows(game, [wired[0]], wired)))
        if counts[0] == 1 and counts[1] >= 1:
            return {"models": counts[0], "wired_players": counts[1]}
        time.sleep(0.5)
    raise AssertionError(f"character model not wired in {timeout} s: models/wired players {counts}")


def anim_state(game, player, path):
    result = game.call("world.get_components", {"entity": player, "components": [path], "strict": True})
    components = result.get("components", result) if isinstance(result, dict) else result
    return components[path]


def wait_state(game, player, path, expected, timeout=3.0):
    deadline = time.monotonic() + timeout
    state = None
    while time.monotonic() < deadline:
        state = anim_state(game, player, path)
        if state == expected:
            return
        time.sleep(POLL_S)
    raise AssertionError(f"player did not settle to {expected} in {timeout} s: {state}")


def run_phase(game, out, name, keys, hold_ms, observe_ms, start):
    player, _, _ = player_position(game)
    path = game.component_path("AnimState")
    teleport(game, start)
    wait_state(game, player, path, "Idle")
    samples, shots, pending = [], [], []
    game.send_keys(keys, hold_ms)
    t0 = time.monotonic()
    next_shot = 0
    last_shot = None
    while True:
        t_ms = (time.monotonic() - t0) * 1000.0
        if t_ms > observe_ms:
            break
        samples.append({"t_ms": round(t_ms, 1), "state": anim_state(game, player, path),
                        "position": player_position(game)[2]})
        if t_ms >= next_shot:
            shot = out / f"{name}_{len(shots):02d}.png"
            game.screenshot(shot)
            shot_t = (time.monotonic() - t0) * 1000.0
            shots.append({"path": str(shot), "t_ms": round(shot_t, 1),
                          "interval_ms": None if last_shot is None else round(shot_t - last_shot, 1)})
            pending.append(shot)
            last_shot = shot_t
            next_shot = (math.floor(shot_t / SHOT_EVERY_MS) + 1) * SHOT_EVERY_MS
        time.sleep(max(0.0, POLL_S - ((time.monotonic() - t0) * 1000.0 - t_ms) / 1000.0))
    return samples, shots, pending


def horizontal_shift(samples):
    a, b = samples[0]["position"], samples[-1]["position"]
    return math.hypot(b[0] - a[0], b[2] - a[2])


def check_gait(name, expected, speed, samples):
    lo, hi = GAIT_WINDOW_MS
    window = [s for s in samples if lo <= s["t_ms"] <= hi]
    if len(window) < 2:
        raise AssertionError(f"phase {name}: only {len(window)} samples in {lo}..{hi} ms")
    shift = horizontal_shift(window)
    wrong = [s for s in window if s["state"] != expected]
    if not wrong:
        return shift
    needed = 0.8 * speed * (window[-1]["t_ms"] - window[0]["t_ms"]) / 1000.0
    if shift < needed:
        raise AssertionError(f"route blocked in phase {name}: moved {shift:.2f} m, needed {needed:.2f} m")
    raise AssertionError(f"phase {name}: expected {expected} in {lo}..{hi} ms, got {wrong[:5]}")


def check_jump(samples):
    states = [s["state"] for s in samples]
    if "Jump" not in states:
        raise AssertionError(f"jump: no Jump sample: {states}")
    after = states[states.index("Jump"):]
    if "Fall" not in after:
        raise AssertionError(f"jump: no Fall after Jump: {states}")
    end = samples[-1]["t_ms"]
    rest = [s["state"] for s in samples if s["t_ms"] >= end - JUMP_REST_MS]
    if any(state != "Idle" for state in rest):
        raise AssertionError(f"jump: not Idle in the last {JUMP_REST_MS} ms: {rest}")


def cadence_misses(shots):
    """Screenshot gaps that skipped a 200 ms mark: the owner's evidence is incomplete there (AnimState samples still gate)."""
    return [shot for shot in shots if shot["interval_ms"] is not None and shot["interval_ms"] > 2 * SHOT_EVERY_MS]


def verify_png(paths):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline and not all(p.is_file() for p in paths):
        time.sleep(0.25)
    bad = [str(p) for p in paths if not p.is_file() or p.read_bytes()[:8] != PNG]
    if bad:
        raise AssertionError(f"screenshots not published as PNG: {bad}")


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    check = subprocess.run([sys.executable, str(REPO / "tools" / "fetch_assets.py"), "--check"], cwd=REPO)
    if check.returncode != 0:
        raise AssertionError("run python tools/fetch_assets.py first")
    summary = {"seed": SEED, "phases": {}}
    failures = []
    with Game(features=("dev",), args=("--seed", str(SEED)), release=True) as game:
        game.wait_resource("CityLayoutHash", 180)
        deadline = time.monotonic() + 30
        while True:
            try:
                player_position(game)
                break
            except AssertionError:
                if time.monotonic() > deadline:
                    raise
                time.sleep(0.5)
        summary["model"] = wait_model(game)
        park = vec3(resource_value(game, "CityLandmarks")["park_center"])
        start = (park[0], park[1] + PLAYER_LIFT, park[2])
        summary["start"] = start
        face(game, (0.0, -1.0), -15.0)

        pending = []
        for name, keys, hold_ms, observe_ms, expected, speed in GAIT_PHASES:
            samples, shots, files = run_phase(game, out, name, keys, hold_ms, observe_ms, start)
            pending += files
            phase = {"keys": keys, "expected": expected, "samples": samples, "screenshots": shots,
                     "cadence_misses": cadence_misses(shots)}
            summary["phases"][name] = phase
            try:
                phase["window_shift_m"] = check_gait(name, expected, speed, samples)
            except AssertionError as error:
                failures.append(str(error))
        name, keys, hold_ms, observe_ms = JUMP_PHASE
        samples, shots, files = run_phase(game, out, name, keys, hold_ms, observe_ms, start)
        pending += files
        summary["phases"][name] = {"keys": keys, "samples": samples, "screenshots": shots,
                                   "cadence_misses": cadence_misses(shots)}
        try:
            check_jump(samples)
        except AssertionError as error:
            failures.append(str(error))

        verify_png(pending)
        errors = log_errors(game)
        summary["log_errors"] = errors
        if errors:
            failures.append("gltf/asset/animation errors in the game log:\n" + "\n".join(errors))
        game.shutdown()
        game.process.wait(timeout=15)
    summary["failures"] = failures
    (out / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    for name, phase in summary["phases"].items():
        print(name, [(s["t_ms"], s["state"]) for s in phase["samples"]])
    misses = {name: phase["cadence_misses"] for name, phase in summary["phases"].items() if phase["cadence_misses"]}
    print(json.dumps({"model": summary["model"], "screenshot_cadence_misses": misses, "failures": failures}, indent=2))
    if failures:
        raise AssertionError("\n".join(failures))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t4")
    run(parser.parse_args().out.resolve())
