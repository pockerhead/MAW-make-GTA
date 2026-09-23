"""Runtime T5 gate: DebugDamage over BRP drains armour then health, the armour pickup works, death
enters Wasted with the "ПОТРАЧЕНО" screen, and the same player respawns at the hospital while the
city is not rebuilt. Screenshots are evidence for the owner."""

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
DAMAGE = "gta_sim::player::DebugDamage"
GAME_STATE = "State<gta_sim::flow::GameState>"
WASTED_PHASE = "State<gta_sim::flow::WastedPhase>"
ERROR_WORDS = ("font", "asset", "Failed to load")
# respawn.ron: 1.5 s slow motion + 3 s screen of real time; generous bounds for a loaded machine.
WASTED_SECONDS = (3.5, 8.0)
SETTLE_S = 0.5


def resource_value(game, suffix):
    value = game.call("world.get_resources", {"resource": game.resource_path(suffix)})["value"]
    if isinstance(value, list) and len(value) == 1:
        value = value[0]
    return value


def state_name(raw):
    """`State<S>` is a newtype; BRP may give the variant as a string, a list or a one-field map."""
    while not isinstance(raw, str):
        if isinstance(raw, list) and len(raw) == 1:
            raw = raw[0]
        elif isinstance(raw, dict) and len(raw) == 1:
            raw = next(iter(raw.values()))
        else:
            raise AssertionError(f"unexpected state value {raw!r}")
    return raw


def game_state(game):
    return state_name(resource_value(game, GAME_STATE))


def wasted_phase(game):
    """The phase, or None while `State<WastedPhase>` does not exist (outside Wasted)."""
    try:
        return state_name(resource_value(game, WASTED_PHASE))
    except RuntimeError:
        return None


def poll(what, probe, timeout, interval=0.05):
    deadline = time.monotonic() + timeout
    value = None
    while time.monotonic() < deadline:
        value = probe()
        if value:
            return value
        time.sleep(interval)
    raise AssertionError(f"{what} not reached in {timeout} s (last {value!r})")


def published(path):
    return path.is_file() and path.read_bytes()[:8] == PNG


def screenshot(game, path):
    # A PNG left by an earlier run in the same --out would otherwise pass as this capture.
    path.unlink(missing_ok=True)
    game.screenshot(path)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline and not published(path):
        time.sleep(0.05)
    if not published(path):
        raise AssertionError(f"screenshot {path} was not published as PNG")
    return str(path)


def player(game):
    marker = game.component_path("Player")
    paths = [game.component_path(name) for name in ("Position", "Health", "CharacterBody")]
    found = game.query(paths, with_=[marker])
    if len(found) != 1:
        raise AssertionError(f"expected one Player row, got {len(found)}")
    row = found[0]
    position, health, body = (row["components"][p] for p in paths)
    return {
        "entity": row["entity"],
        "position_path": paths[0],
        "position": vec3(position),
        "health": (health["current"], health["armor"]),
        "float_height": body["float_height"],
    }


def pickups(game):
    pickup = game.component_path("Pickup")
    transform = game.component_path("Transform")
    return {
        row["components"][pickup]["kind"]: {
            "cooldown": row["components"][pickup]["cooldown"],
            "at": vec3(row["components"][transform]["translation"]),
        }
        for row in game.query([pickup, transform], with_=[pickup])
    }


def damage(game, amount):
    game.call("world.write_message", {"message": DAMAGE, "value": {"amount": amount}})


def chunk_count(game):
    chunk = game.component_path("CityChunk")
    return len(game.query([chunk], with_=[chunk]))


def wait_chunks(game, timeout=60):
    """City chunk count once two readings a second apart agree."""
    deadline = time.monotonic() + timeout
    last = -1
    while time.monotonic() < deadline:
        count = chunk_count(game)
        if count > 0 and count == last:
            return count
        last = count
        time.sleep(1.0)
    raise AssertionError(f"city chunk count did not settle in {timeout} s (last {last})")


def horizontal(a, b):
    return math.hypot(a[0] - b[0], a[2] - b[2])


def close(actual, expected, what):
    if any(abs(a - e) > 1e-3 for a, e in zip(actual, expected)):
        raise AssertionError(f"{what}: health/armour {actual}, expected {expected}")


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
    try:
        with Game(features=("dev",), args=("--seed", str(SEED)), release=True) as game:
            layout_hash = game.wait_resource("CityLayoutHash", 180)
            if layout_hash != load_golden()[SEED]:
                raise AssertionError(f"layout hash {layout_hash:#x} is not the golden hash of seed {SEED}")
            chunks0 = wait_chunks(game)
            summary["chunks_before"] = chunks0

            hospital = resource_value(game, "HospitalSpawn")
            point = vec3(hospital["point"])
            summary["hospital"] = {"point": point, "along": vec3(hospital["along"])}
            items = pickups(game)
            summary["pickups"] = items
            if sorted(items) != ["Armor", "Health"]:
                raise AssertionError(f"expected one Health and one Armor pickup, got {sorted(items)}")
            me = player(game)
            close(me["health"], (100.0, 0.0), "at start")

            damage(game, 40.0)
            time.sleep(SETTLE_S)
            close(player(game)["health"], (60.0, 0.0), "after 40 damage")
            summary["hud_damaged"] = screenshot(game, out / "hud_damaged.png")

            armor = items["Armor"]["at"]
            target = [armor[0], armor[1] + me["float_height"], armor[2]]
            game.mutate_component(me["entity"], me["position_path"], "", target)
            time.sleep(SETTLE_S)
            close(player(game)["health"], (60.0, 50.0), "on the armour pickup")
            damage(game, 30.0)
            time.sleep(SETTLE_S)
            close(player(game)["health"], (60.0, 20.0), "armour absorbs 30 damage")

            damage(game, 1000.0)
            poll("GameState Wasted", lambda: game_state(game) == "Wasted", 2.0)
            wasted_at = time.monotonic()
            poll("WastedPhase Screen", lambda: wasted_phase(game) == "Screen", 5.0)
            summary["wasted_screen"] = screenshot(game, out / "wasted.png")
            still = game_state(game)
            summary["state_after_wasted_screenshot"] = still
            summary["raw_game_state"] = resource_value(game, GAME_STATE)
            if still != "Wasted":
                raise AssertionError(f"the wasted screenshot came too late: state is {still}")

            poll("GameState Playing", lambda: game_state(game) == "Playing", 10.0)
            seconds = time.monotonic() - wasted_at
            summary["wasted_seconds"] = round(seconds, 2)
            if not WASTED_SECONDS[0] <= seconds <= WASTED_SECONDS[1]:
                raise AssertionError(f"Wasted lasted {seconds:.2f} s, expected {WASTED_SECONDS}")
            back = player(game)
            summary["respawn"] = {"position": back["position"], "health": back["health"]}
            if back["entity"] != me["entity"]:
                raise AssertionError(f"respawn made a new player entity {back['entity']} (was {me['entity']})")
            distance = horizontal(back["position"], point)
            summary["respawn"]["distance_m"] = round(distance, 3)
            if distance >= 1.0:
                raise AssertionError(f"respawned {distance:.2f} m from the hospital point {point}")
            close(back["health"], (100.0, 0.0), "after respawn")
            time.sleep(1.0)
            summary["respawned"] = screenshot(game, out / "respawned.png")

            time.sleep(5.0)
            chunks1 = chunk_count(game)
            summary["chunks_after"] = chunks1
            if chunks1 != chunks0:
                raise AssertionError(f"city chunks {chunks0} -> {chunks1} across respawn")
            player(game)

            errors = log_errors(game)
            summary["log_errors"] = errors
            if errors:
                raise AssertionError("font/asset errors in the game log:\n" + "\n".join(errors))
            game.shutdown()
            game.process.wait(timeout=15)
    finally:
        (out / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(summary, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=REPO / "target" / "qa" / "t5")
    run(parser.parse_args().out.resolve())
