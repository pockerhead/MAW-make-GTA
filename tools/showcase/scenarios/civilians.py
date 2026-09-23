"""Civilians: a run down a busy street, then a pistol shot into the air scatters the passers-by."""

import math
import time

from _common import look, orbit, pan, stand
from t6 import player, weapon_pickups
from t8 import alive, civilians, counts, horizontal

CAPTION = "Мирные жители: живая улица на бегу, после выстрела в воздух разбегаются"
ANCHOR = "- TASK-009 (T8)"
SEED = 1
# Streets of the city grid run along the axes; the orbit camera yaw 0 looks along -Z.
AXIS_YAWS_DEG = (0.0, 90.0, 180.0, 270.0)
START_BEHIND_M = 8.0
REFILL_S = 6.0
VIEW_PITCH_DEG = -6.0
SHOT_PITCH_DEG = 20.0
RUN_MS = 4000
FILL_TIMEOUT_S = 40.0
FILL_SHARE = 0.9
MAX_CIVILIANS = 40
AHEAD_RANGE_M = (3.0, 40.0)
AHEAD_HALF_DEG = 15.0
FOLLOW_RADIUS_M = 40.0


def ahead(people, at, yaw_deg):
    """Civilians within `AHEAD_RANGE_M` of `at` inside a horizontal cone around `yaw_deg`."""
    seen = 0
    for c in people:
        dx, dz = c["position"][0] - at[0], c["position"][2] - at[2]
        if not AHEAD_RANGE_M[0] <= math.hypot(dx, dz) <= AHEAD_RANGE_M[1]:
            continue
        # Orbit camera looks along -Z rotated by yaw (camera/mod.rs, EulerRot::YXZ).
        bearing = math.degrees(math.atan2(-dx, -dz))
        if abs((bearing - yaw_deg + 180.0) % 360.0 - 180.0) <= AHEAD_HALF_DEG:
            seen += 1
    return seen


def best_street(people):
    """(count, start, yaw): a start a few metres behind a passer-by, looking along a street axis with the most civilians ahead."""
    best = (-1, None, 0.0)
    for c in people:
        for yaw in AXIS_YAWS_DEG:
            back = math.radians(yaw)
            start = [c["position"][0] + math.sin(back) * START_BEHIND_M, c["position"][1], c["position"][2] + math.cos(back) * START_BEHIND_M]
            seen = ahead(people, start, yaw)
            if seen > best[0]:
                best = (seen, start, yaw)
    return best


def prepare(game):
    gun = next(i for i in weapon_pickups(game) if i["weapon"] == "Pistol" and not i["ammo_only"])
    stand(game, gun["at"], settle=0.4)
    # Civilians spawn ahead of the camera and behind buildings; wait until the bubble is near the cap.
    deadline = time.monotonic() + FILL_TIMEOUT_S
    while len(alive(civilians(game))) < MAX_CIVILIANS * FILL_SHARE and time.monotonic() < deadline:
        time.sleep(0.5)
    seen, start, yaw = best_street(alive(civilians(game)))
    print(f"{seen} civilians ahead of the chosen start, yaw {yaw}")
    me = player(game)
    stand(game, [start[0], start[1] - me["float_height"], start[2]], settle=0.3)
    look(game, yaw, VIEW_PITCH_DEG)
    # Let recycling bring the calm civilians from behind the view to the street ahead.
    time.sleep(REFILL_S)
    people = alive(civilians(game))
    print(f"{len(people)} civilians, {ahead(people, player(game)['position'], yaw)} ahead after the refill")


def scared_yaw(game, fallback):
    """Bearing from the player to the middle of the nearby civilians that react to the shot."""
    me = player(game)["position"]
    scared = [
        c for c in alive(civilians(game))
        if c["state"] in ("Flee", "Report") and horizontal(c["position"], me) < FOLLOW_RADIUS_M
    ]
    if not scared:
        return fallback
    x = sum(c["position"][0] for c in scared) / len(scared)
    z = sum(c["position"][2] for c in scared) / len(scared)
    return math.degrees(math.atan2(-(x - me[0]), -(z - me[2])))


def play(game):
    game.send_keys(["KeyW"], RUN_MS)
    time.sleep(RUN_MS / 1000.0 + 0.2)
    yaw = math.degrees(orbit(game)[2]["yaw"])
    people = alive(civilians(game))
    print(f"after the run: {len(people)} civilians, {ahead(people, player(game)['position'], yaw)} ahead")
    pan(game, yaw, SHOT_PITCH_DEG, 0.3)
    for _ in range(2):
        game.send_mouse_button("Left", 80)
        time.sleep(0.4)
    pan(game, yaw, VIEW_PITCH_DEG, 0.4)
    print("states after the shot:", counts(alive(civilians(game))))
    target = scared_yaw(game, yaw)
    pan(game, yaw + ((target - yaw + 180.0) % 360.0 - 180.0) * 0.3, VIEW_PITCH_DEG - 2.0, 2.2)
    print("states at the end:", counts(alive(civilians(game))))
