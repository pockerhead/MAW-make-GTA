"""Runtime T1 liveness gate: input, camera, screenshot, FPS, shutdown."""

import argparse
import json
from pathlib import Path
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from brp import Game, REPO, vec3  # noqa: E402


def only_row(game, component, marker):
    rows = game.query([component], with_=[marker])
    if len(rows) != 1:
        raise RuntimeError(f"expected one row with {marker}; got {len(rows)}")
    return rows[0]["components"][component]


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    summary = {}
    with Game(features=("dev",)) as game:
        time.sleep(2)
        player = game.component_path("Player")
        transform = game.component_path("Transform")
        start = vec3(only_row(game, transform, player)["translation"])
        game.send_keys(["KeyW"], 1000)
        time.sleep(1.6)
        end = vec3(only_row(game, transform, player)["translation"])
        delta = tuple(b - a for a, b in zip(start, end))
        if not (delta[2] < -2.0 and abs(delta[0]) < 0.5):
            raise AssertionError(f"W did not move the player along -Z: {delta}")
        summary["movement_delta"] = delta

        orbit = game.component_path("OrbitCamera")
        yaw0 = only_row(game, orbit, orbit)["yaw"]
        game.move_mouse(200, 0)
        time.sleep(0.35)
        yaw1 = only_row(game, orbit, orbit)["yaw"]
        change = yaw1 - yaw0
        if not -0.6 < change < -0.2:
            raise AssertionError(f"camera yaw delta out of range: {change}")
        summary["yaw_delta"] = change

        image = out / "t1.png"
        game.screenshot(image)
        if not image.is_file() or image.read_bytes()[:8] != b"\x89PNG\r\n\x1a\n":
            raise AssertionError("screenshot was not published as PNG")
        summary["screenshot"] = str(image)

        fps = game.diagnostics()["fps"]["current"]
        if fps is None or fps <= 0:
            raise AssertionError(f"FPS unavailable: {fps}")
        summary["fps"] = fps
        summary["window_state_at_fps"] = game.window_state()
        game.shutdown()
        game.process.wait(timeout=15)
        summary["shutdown"] = "passed"
    (out / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t1")
    run(parser.parse_args().out)
