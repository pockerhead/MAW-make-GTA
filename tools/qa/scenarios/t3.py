"""Runtime T3 gate: merged city chunks and props spawn, the tower roof holds the player,
screenshots from the roof and the park, FPS evidence for the owner, no shader/asset errors."""

import argparse
import json
import math
from pathlib import Path
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from brp import Game, REPO, load_golden, vec3  # noqa: E402

PNG = b"\x89PNG\r\n\x1a\n"
SEED = 1
EXPECTED_CHUNKS = 100
PLAYER_LIFT = 1.2
ERROR_WORDS = ("wgsl", "shader", "gltf", "asset", "Failed to load")
# Inside the top roof tier for any tower yaw: its half extent is >= massing.setback_min_half (6 m).
ROOF_EDGE_OFFSET = 4.5
OVERVIEW_PITCH_DEG = -45.0
OUTWARD = {"east": (1.0, 0.0), "south": (0.0, 1.0), "west": (-1.0, 0.0), "north": (0.0, -1.0)}


def screenshot(game, path):
    game.screenshot(path)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline and not path.is_file():
        time.sleep(0.25)
    if not path.is_file() or path.read_bytes()[:8] != PNG:
        raise AssertionError(f"screenshot {path} was not published as PNG")
    return str(path)


def rows(game, marker):
    return len(game.query([marker], with_=[marker]))


def wait_city(game, timeout=60):
    chunk = game.component_path("CityChunk")
    prop = game.component_path("CityProp")
    deadline = time.monotonic() + timeout
    last = (-1, -1)
    while time.monotonic() < deadline:
        counts = (rows(game, chunk), rows(game, prop))
        if counts[0] == EXPECTED_CHUNKS and counts == last and counts[1] > 0:
            return {"chunks": counts[0], "props": counts[1]}
        last = counts
        time.sleep(1.0)
    raise AssertionError(f"city not spawned in {timeout} s: chunks/props {last}")


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


def fps_samples(game, count=5):
    samples = []
    for _ in range(count):
        samples.append(game.diagnostics())
        time.sleep(1.0)
    fps = [s["fps"]["current"] for s in samples if s.get("fps", {}).get("current")]
    frame = [s["frame_time_ms"]["current"] for s in samples if s.get("frame_time_ms", {}).get("current")]
    return {
        "fps_min": min(fps) if fps else None,
        "fps_avg": sum(fps) / len(fps) if fps else None,
        "frame_time_ms_max": max(frame) if frame else None,
        "raw": samples,
    }


def log_errors(game):
    lines = game.log_tail(2_000_000).splitlines()
    return [
        line for line in lines
        if "ERROR" in line and any(word.lower() in line.lower() for word in ERROR_WORDS)
    ]


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    check = subprocess.run([sys.executable, str(REPO / "tools" / "fetch_assets.py"), "--check"], cwd=REPO)
    if check.returncode != 0:
        raise AssertionError("run python tools/fetch_assets.py first")
    summary = {"seed": SEED}
    with Game(features=("dev",), args=("--seed", str(SEED)), release=True) as game:
        layout_hash = game.wait_resource("CityLayoutHash", 180)
        golden = load_golden()[SEED]
        if layout_hash != golden:
            raise AssertionError(f"CityLayoutHash {layout_hash:#018x} != golden {golden:#018x}")
        summary["hash"] = f"{layout_hash:#018x}"
        summary["spawned"] = wait_city(game)

        landmarks = resource_value(game, "CityLandmarks")
        roof = vec3(landmarks["tower_roof"])
        park = vec3(landmarks["park_center"])
        summary["landmarks"] = {key: vec3(value) for key, value in landmarks.items()}

        shots = []
        positions = {}
        for name, (dx, dz) in OUTWARD.items():
            target = (roof[0] + dx * ROOF_EDGE_OFFSET, roof[1] + PLAYER_LIFT, roof[2] + dz * ROOF_EDGE_OFFSET)
            at = teleport(game, target)
            if not roof[1] + 0.8 < at[1] < roof[1] + 1.4:
                raise AssertionError(f"player not standing on the tower roof at y {roof[1]} ({name}): {at}")
            positions[name] = at
            face(game, (dx, dz), OVERVIEW_PITCH_DEG)
            shots.append(screenshot(game, out / f"roof_{name}.png"))
        summary["roof_positions"] = positions
        summary["roof_screenshots"] = shots

        # FPS while the camera looks over the roof edge at the city (last direction above).
        summary["roof_fps_view"] = {"facing": name, "pitch_deg": OVERVIEW_PITCH_DEG}
        time.sleep(5.0)
        summary["roof_fps"] = fps_samples(game)
        summary["window_state"] = game.window_state()

        summary["park_position"] = teleport(game, (park[0], park[1] + PLAYER_LIFT, park[2]))
        summary["park_screenshot"] = screenshot(game, out / "park.png")

        errors = log_errors(game)
        summary["log_errors"] = errors
        game.shutdown()
        game.process.wait(timeout=15)
    (out / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps({k: v for k, v in summary.items() if k != "roof_fps"}, indent=2))
    print(json.dumps({k: v for k, v in summary["roof_fps"].items() if k != "raw"}, indent=2))
    if errors:
        raise AssertionError("shader/asset errors in the game log:\n" + "\n".join(errors))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t3")
    run(parser.parse_args().out.resolve())
