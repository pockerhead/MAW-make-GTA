"""Car: the player runs up to a parked sedan, presses F and drives off down the avenue, hops the curb onto
a sidewalk with passers-by (they scatter) and crashes into a building (metal hit, camera shake)."""

import math
import time

from _common import look, stand
from t8 import alive, civilians
from t6 import player as walker
from t14 import car, cars, driving, flat, player, rotate

CAPTION = "Машина: садимся в припаркованный седан на F, разгон по проспекту, через бордюр к прохожим (разбегаются) и удар в стену с тряской"
ANCHOR = "- TASK-015 (T14)"
SEED = 1
# Parked cars stand in the outer avenue lane, 1.625 m left of the curb (world/city.ron), the sidewalk
# is 4 m wide beyond it; a spot is scored by the passers-by on that sidewalk ahead.
SIDEWALK_RIGHT_M = (1.8, 7.0)
AHEAD_M = (15.0, 45.0)
LANE_CLEAR_M = 45.0
CANDIDATE_RADIUS_M = 150.0
FILL_TIMEOUT_S = 40.0
FILL_COUNT = 34
REFILL_S = 5.0
START_BEHIND_M = 5.0
VIEW_PITCH_DEG = -10.0
ENTER_AT_M = 1.2
DRIVE_MS = 8500
# The curb hop starts when passers-by are this far ahead of the car on its sidewalk, but not before
# HOP_EARLIEST_S (speed) nor after HOP_LATEST_S (the clip length).
HOP_AHEAD_M = (10.0, 28.0)
HOP_EARLIEST_S = 1.8
HOP_LATEST_S = 4.0
# Short D pulses until the car runs this many degrees off the lane, then straight into the building.
TURN_DEG = 24.0
PULSE_MS = 120
AFTER_CRASH_S = 1.8
PLAY_LIMIT_S = 11.3
SCENE = {}


def frame(spot):
    """Unit forward and right (x, z) of a parked car; Rotation turns body -Z into the heading."""
    f = rotate(spot["rotation"], (0.0, 0.0, -1.0))
    forward = (f[0], f[2])
    return forward, (-forward[1], forward[0])


def local(spot, at):
    forward, right = frame(spot)
    dx, dz = at[0] - spot["position"][0], at[2] - spot["position"][2]
    return dx * forward[0] + dz * forward[1], dx * right[0] + dz * right[1]


def score(spot, people, parked):
    for other in parked:
        along, side = local(spot, other["position"])
        if other["entity"] != spot["entity"] and 0.0 < along < LANE_CLEAR_M and abs(side) < 3.0:
            return -1
    seen = 0
    for c in people:
        along, side = local(spot, c["position"])
        if AHEAD_M[0] <= along <= AHEAD_M[1] and SIDEWALK_RIGHT_M[0] <= side <= SIDEWALK_RIGHT_M[1]:
            seen += 1
    return seen


def best_spot(game):
    me = player(game)["position"]
    parked = cars(game)
    people = alive(civilians(game))
    near = [c for c in parked if flat(c["position"], me) < CANDIDATE_RADIUS_M]
    return max(((score(c, people, parked), c) for c in near), key=lambda row: row[0])


def yaw_deg(direction):
    return math.degrees(math.atan2(-direction[0], -direction[1]))


def prepare(game):
    deadline = time.monotonic() + FILL_TIMEOUT_S
    while len(alive(civilians(game))) < FILL_COUNT and time.monotonic() < deadline:
        time.sleep(0.5)
    seen, spot = best_spot(game)
    print(f"parked car {spot['entity']}: {seen} passers-by on the sidewalk ahead")
    forward, _ = frame(spot)
    door = rotate(spot["rotation"], (-1.7, 0.0, -0.3))
    at = (spot["position"][0] + door[0] - forward[0] * START_BEHIND_M,
          spot["position"][2] + door[2] - forward[1] * START_BEHIND_M)
    me = walker(game)
    feet_y = me["position"][1] - me["float_height"]
    stand(game, [at[0], feet_y, at[1]], settle=0.3)
    SCENE["spot"] = spot
    SCENE["door"] = (spot["position"][0] + door[0], 0.0, spot["position"][2] + door[2])
    look(game, yaw_deg(forward), VIEW_PITCH_DEG)
    time.sleep(REFILL_S)
    people = alive(civilians(game))
    print(f"{score(spot, people, cars(game))} passers-by ahead after the refill")
    look(game, yaw_deg(forward), VIEW_PITCH_DEG)


def play(game):
    spot = SCENE["spot"]
    started = time.monotonic()
    # One W hold for the run up and the drive: in the car it becomes the throttle.
    game.send_keys(["KeyW"], DRIVE_MS)
    while flat(player(game)["position"], SCENE["door"]) > ENTER_AT_M:
        if time.monotonic() - started > 3.0:
            raise TimeoutError("did not reach the door")
        time.sleep(0.02)
    game.send_keys(["KeyF"], 100)
    while driving(game) != spot["entity"]:
        if time.monotonic() - started > 4.0:
            raise TimeoutError("did not get in")
        time.sleep(0.02)
    entered = time.monotonic()
    health = car(game, spot["entity"])["health"]
    phase = "lane"
    crashed = None
    while time.monotonic() - started < PLAY_LIMIT_S:
        now = car(game, spot["entity"])
        since = time.monotonic() - entered
        if phase == "lane" and since >= HOP_EARLIEST_S:
            along = local(spot, now["position"])[0]
            ahead = [c for c in alive(civilians(game))
                     if HOP_AHEAD_M[0] <= local(spot, c["position"])[0] - along <= HOP_AHEAD_M[1]
                     and SIDEWALK_RIGHT_M[0] <= local(spot, c["position"])[1] <= SIDEWALK_RIGHT_M[1]]
            if ahead or since >= HOP_LATEST_S:
                print(f"hop {since:.1f} s after entry, {len(ahead)} passers-by ahead")
                phase = "turn"
        if phase == "turn":
            forward, right = frame(spot)
            v = now["velocity"]
            off = math.degrees(math.atan2(v[0] * right[0] + v[2] * right[1], v[0] * forward[0] + v[2] * forward[1]))
            if off >= TURN_DEG:
                phase = "wall"
            elif game.held_until.get("KeyD", 0.0) <= time.monotonic():
                game.send_keys(["KeyD"], PULSE_MS)
        if crashed is None and now["health"] < health - 1.0:
            crashed = time.monotonic()
            print(f"crash {time.monotonic() - entered:.1f} s after entry, health {health:.0f} -> {now['health']:.0f}")
        if crashed is not None and time.monotonic() - crashed > AFTER_CRASH_S:
            break
        time.sleep(0.05)
    if crashed is None:
        print("no crash")
