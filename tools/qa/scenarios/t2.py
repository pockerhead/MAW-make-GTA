"""Runtime T2 gate: --seed wiring, CityLayoutHash == golden, teleport over BRP, ground screenshots."""

import argparse
import json
from pathlib import Path
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from brp import Game, REPO, load_golden, vec3  # noqa: E402

PNG = b"\x89PNG\r\n\x1a\n"
TELEPORT = [0.0, 1.2, 0.0]


def screenshot(game, path):
    game.screenshot(path)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline and not path.is_file():
        time.sleep(0.25)
    if not path.is_file() or path.read_bytes()[:8] != PNG:
        raise AssertionError(f"screenshot {path} was not published as PNG")
    return str(path)


def player_row(game, player, components, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        rows = game.query(components, with_=[player])
        if len(rows) == 1:
            return rows[0]
        time.sleep(0.25)
    raise TimeoutError(f"expected one Player row within {timeout} s")


def teleport(game, entity, position):
    """Tries the Position value shapes in order and returns the one BRP accepted."""
    attempts = [("", TELEPORT), (".0", TELEPORT), ("", dict(zip("xyz", TELEPORT)))]
    errors = []
    for path, value in attempts:
        try:
            game.mutate_component(entity, position, path, value)
            return {"path": path, "value": value}
        except RuntimeError as error:
            errors.append(f"path={path!r} value={value!r}: {error}")
    raise AssertionError("world.mutate_components rejected every Position form:\n" + "\n".join(errors))


def check_standing(name, at):
    x, y, z = at
    if not (abs(x) < 1.0 and abs(z) < 1.0 and 0.9 < y < 1.3):
        raise AssertionError(f"{name} after teleport is not standing at the centre: {at}")


def run_seed(seed, golden, out):
    result = {"seed": seed}
    with Game(features=("dev",), args=("--seed", str(seed))) as game:
        layout_hash = game.wait_resource("CityLayoutHash", 180)
        city_seed = game.resource("CitySeed")
        if city_seed != seed:
            raise AssertionError(f"CitySeed {city_seed} != --seed {seed}")
        if layout_hash != golden[seed]:
            raise AssertionError(f"seed {seed}: CityLayoutHash {layout_hash:#018x} != golden {golden[seed]:#018x}")
        result["hash"] = f"{layout_hash:#018x}"

        player = game.component_path("Player")
        position = game.component_path("Position")
        transform = game.component_path("Transform")
        row = player_row(game, player, [position, transform])
        time.sleep(1.5)
        result["spawn"] = vec3(player_row(game, player, [transform])["components"][transform]["translation"])
        result["spawn_screenshot"] = screenshot(game, out / f"spawn_{seed}.png")

        result["mutation_form"] = teleport(game, row["entity"], position)
        time.sleep(1.5)
        row = player_row(game, player, [position, transform])
        at_position = vec3(row["components"][position])
        at_transform = vec3(row["components"][transform]["translation"])
        check_standing("Position", at_position)
        check_standing("Transform", at_transform)
        result["center_position"] = at_position
        result["center_transform"] = at_transform
        result["center_screenshot"] = screenshot(game, out / f"center_{seed}.png")

        result["fps"] = game.diagnostics()["fps"]["current"]
        game.shutdown()
        game.process.wait(timeout=15)
    return result


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    golden = load_golden()
    runs = [run_seed(seed, golden, out) for seed in (1, 2)]
    if runs[0]["hash"] == runs[1]["hash"]:
        raise AssertionError(f"seeds 1 and 2 produced the same layout hash {runs[0]['hash']}")
    summary = {"runs": runs, "hashes_differ": True}
    (out / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t2")
    run(parser.parse_args().out.resolve())
