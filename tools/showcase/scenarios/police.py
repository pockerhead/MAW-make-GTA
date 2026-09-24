"""Police: one star over BRP, the stars blink, two patrol cops run up on foot, the arrest and "BUSTED"."""

import math
import time

from _common import look, pan, stand
from t5 import game_state, resource_value
from t6 import player
from t8 import horizontal
from t11 import ARMOR, busted_phase, cops, set_heat, star_heats

CAPTION = "Полиция: звезда розыска мигает, патрульные прибегают пешком, арест и экран \"BUSTED\""
ANCHOR = "- TASK-012 (T11)"
SEED = 1
# Cops spawn 40..90 m away (escalation.ron spawn_ring): too far for a 12 s clip, so the first units
# are moved this far down the sidewalk, behind the camera, and filmed as they run in.
COP_M = 14.0
COP_SPREAD_M = 2.5
SPAWN_TIMEOUT_S = 3.0
VIEW_PITCH_DEG = -10.0
SIDE_YAW_DEG = 35.0
DRIFT_YAW_DEG = 12.0
# Station sidewalk: feet point and unit direction along it, set by prepare().
SPOT = {}


def yaw_along(d):
    """Orbit camera yaw looking along `d` (yaw 0 looks along -Z)."""
    return math.degrees(math.atan2(-d[0], -d[2]))


def prepare(game):
    spawn = resource_value(game, "PoliceStationSpawn")
    feet = [spawn["point"][k] for k in "xyz"] if isinstance(spawn["point"], dict) else list(spawn["point"])
    along = [spawn["along"][k] for k in "xyz"] if isinstance(spawn["along"], dict) else list(spawn["along"])
    stand(game, feet, settle=0.5)
    me = player(game)
    game.mutate_component(me["entity"], game.component_path("Health"), ".armor", ARMOR)
    look(game, yaw_along([-along[0], 0.0, -along[2]]), VIEW_PITCH_DEG)
    SPOT.update(feet=feet, along=along)
    time.sleep(1.0)


def bring_cops(game):
    """Move the first units COP_M down the sidewalk (+along); each is re-queried right before its move."""
    deadline = time.monotonic() + SPAWN_TIMEOUT_S
    moved = set()
    me = player(game)
    while len(moved) < 2 and time.monotonic() < deadline:
        for c in cops(game):
            if c["entity"] in moved or c["state"] in ("Dead", "Leave"):
                continue
            if not any(x["entity"] == c["entity"] for x in cops(game)):
                continue
            feet, along = SPOT["feet"], SPOT["along"]
            side = COP_SPREAD_M * (0.5 if not moved else -0.5)
            normal = [-along[2], along[0]]
            dist = COP_M + 2.0 * len(moved)
            target = [feet[0] + along[0] * dist + normal[0] * side,
                      feet[1] + me["float_height"],
                      feet[2] + along[2] * dist + normal[1] * side]
            game.mutate_component(c["entity"], game.component_path("Position"), "", target)
            moved.add(c["entity"])
        time.sleep(0.05)
    print(f"moved {len(moved)} cops")


def play(game):
    heat = star_heats()[0]
    yaw = yaw_along(SPOT["along"])
    set_heat(game, heat)
    # Unseen for a moment (the units spawn off-frame 40+ m away): the star blinks.
    pan(game, yaw + 180.0 - DRIFT_YAW_DEG, VIEW_PITCH_DEG, 1.5)
    bring_cops(game)
    pan(game, yaw + SIDE_YAW_DEG, VIEW_PITCH_DEG, 1.8)
    start = time.monotonic()
    while game_state(game) != "Busted" and time.monotonic() - start < 6.0:
        time.sleep(0.1)
    at = player(game)["position"]
    print("busted after", round(time.monotonic() - start, 1), "s;",
          [(c["state"], round(horizontal(c["position"], at), 1)) for c in cops(game)])
    pan(game, yaw + SIDE_YAW_DEG * 1.8, -18.0, 2.0)
    deadline = time.monotonic() + 4.0
    while busted_phase(game) != "Screen" and time.monotonic() < deadline:
        time.sleep(0.1)
    # The BUSTED screen lasts 3 s (respawn.ron); leave before the respawn cut.
    time.sleep(1.9)
