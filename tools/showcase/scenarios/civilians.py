"""Civilians: a walk along a sidewalk toward a group of passers-by, then a pistol shot over their heads scatters them."""

import math
import time

from _common import look, orbit, pan, stand
from t6 import player, weapon_pickups
from t8 import alive, civilians, counts, horizontal

CAPTION = "Мирные жители: гуляют по тротуарам, после выстрела в воздух разбегаются и прячутся"
ANCHOR = "- TASK-009 (T8)"
SEED = 1
VIEW_RANGE_M = (4.0, 16.0)
VIEW_HALF_DEG = 40.0
STAND_OFF_M = 1.5
SHOT_PITCH_DEG = 20.0
VIEW_PITCH_DEG = -8.0
FOLLOW_RADIUS_M = 30.0


def in_view(people, at, yaw_deg):
    """Civilians a few metres ahead of `at` inside a narrow horizontal cone around `yaw_deg`."""
    seen = 0
    for c in people:
        dx, dz = c["position"][0] - at[0], c["position"][2] - at[2]
        if not VIEW_RANGE_M[0] <= math.hypot(dx, dz) <= VIEW_RANGE_M[1]:
            continue
        # Orbit camera looks along -Z rotated by yaw (camera/mod.rs, EulerRot::YXZ).
        bearing = math.degrees(math.atan2(-dx, -dz))
        if abs((bearing - yaw_deg + 180.0) % 360.0 - 180.0) <= VIEW_HALF_DEG:
            seen += 1
    return seen


def prepare(game):
    # The first fill puts civilians around the spawn point; later ones only appear 60+ m away out of
    # view, so the crowd is densest right after loading. Grab the pistol and come straight back.
    me = player(game)
    home = [me["position"][0], me["position"][1] - me["float_height"], me["position"][2]]
    gun = next(i for i in weapon_pickups(game) if i["weapon"] == "Pistol" and not i["ammo_only"])
    stand(game, gun["at"], settle=0.4)
    stand(game, home, settle=0.2)
    # Stand where a passer-by stands (a sidewalk) and face the direction with the most civilians close ahead.
    people = alive(civilians(game))
    seen, at, yaw = max((in_view(people, c["position"], yaw), c["position"], yaw) for c in people for yaw in range(0, 360, 10))
    print(f"{seen} civilians in the opening view")
    me = player(game)
    back = math.radians(yaw)
    feet = [at[0] + math.sin(back) * STAND_OFF_M, at[1] - me["float_height"], at[2] + math.cos(back) * STAND_OFF_M]
    stand(game, feet, settle=0.8)
    look(game, float(yaw), VIEW_PITCH_DEG)
    time.sleep(0.5)


def fleeing_yaw(game, fallback):
    """Bearing from the player to the middle of the nearby civilians that run."""
    me = player(game)["position"]
    scared = [c for c in alive(civilians(game)) if c["state"] == "Flee" and horizontal(c["position"], me) < FOLLOW_RADIUS_M]
    if not scared:
        return fallback
    x = sum(c["position"][0] for c in scared) / len(scared)
    z = sum(c["position"][2] for c in scared) / len(scared)
    return math.degrees(math.atan2(-(x - me[0]), -(z - me[2])))


def play(game):
    game.send_keys(["KeyW"], 1500)
    time.sleep(1.3)
    yaw = math.degrees(orbit(game)[2]["yaw"])
    pan(game, yaw, SHOT_PITCH_DEG, 0.3)
    for _ in range(2):
        game.send_mouse_button("Left", 80)
        time.sleep(0.4)
    pan(game, yaw, VIEW_PITCH_DEG, 0.4)
    print("states after the shot:", counts(alive(civilians(game))))
    target = fleeing_yaw(game, yaw)
    pan(game, yaw + ((target - yaw + 180.0) % 360.0 - 180.0) * 0.5, VIEW_PITCH_DEG - 4.0, 3.5)
    time.sleep(0.5)
