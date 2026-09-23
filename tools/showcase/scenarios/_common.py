"""Helpers shared by showcase scenarios (files starting with "_" are not scenarios)."""

import math
import time

from t5 import resource_value
from t6 import player, teleport


def orbit(game):
    path = game.component_path("OrbitCamera")
    found = game.query([path], with_=[path])
    if len(found) != 1:
        raise RuntimeError(f"expected one OrbitCamera row, got {len(found)}")
    return found[0]["entity"], path, found[0]["components"][path]


def look(game, yaw_deg, pitch_deg):
    entity, path, _ = orbit(game)
    game.mutate_component(entity, path, ".yaw", math.radians(yaw_deg))
    game.mutate_component(entity, path, ".pitch", math.radians(pitch_deg))


def pan(game, yaw_deg, pitch_deg, seconds, rate=30):
    """Smoothly turn the orbit camera from its current view to (yaw, pitch) in `seconds`."""
    entity, path, value = orbit(game)
    yaw0, pitch0 = math.degrees(value["yaw"]), math.degrees(value["pitch"])
    start = time.monotonic()
    while True:
        t = min(1.0, (time.monotonic() - start) / seconds)
        ease = t * t * (3.0 - 2.0 * t)
        game.mutate_component(entity, path, ".yaw", math.radians(yaw0 + (yaw_deg - yaw0) * ease))
        game.mutate_component(entity, path, ".pitch", math.radians(pitch0 + (pitch_deg - pitch0) * ease))
        if t >= 1.0:
            return
        time.sleep(1.0 / rate)


def landmark(game, name):
    value = resource_value(game, "CityLandmarks")[name]
    return [value["x"], value["y"], value["z"]] if isinstance(value, dict) else list(value)


def stand(game, feet, settle=0.6):
    """Teleport the player so its feet are at `feet`, then let the controller settle."""
    teleport(game, player(game), feet)
    time.sleep(settle)


def tilt(game, pitch_deg):
    """Set only the camera pitch (melee swings follow the aim yaw, so the yaw must stay)."""
    entity, path, _ = orbit(game)
    game.mutate_component(entity, path, ".pitch", math.radians(pitch_deg))
