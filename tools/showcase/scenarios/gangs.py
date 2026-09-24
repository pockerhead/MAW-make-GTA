"""Gangs: a run up to a purple gang group at its HQ, a pistol shot into the air, then a firefight."""

import math
import time

from _common import look, orbit, pan, stand
from t6 import player
from t8 import horizontal
from t9 import FIGHT_ARMOR, fighters, gang_posts, members, pistol_pickups, spawn_ring

CAPTION = "Банды: подходим к группе у штаба, выстрел в воздух, и банда открывает огонь"
ANCHOR = "- TASK-010 (T9)"
SEED = 1
GROUP_TIMEOUT_S = 15.0
# Run speed 4.5 m/s: a 1.2 s run ends about 10 m from the group, inside the 15 m shot radius.
START_M = 15.0
RUN_MS = 1200
# After the shot the player pushes on, the members back off to their keep distance.
PUSH_MS = 600
# The start is shifted to the right (the kerb side on seed 1) so the camera does not start inside a wall.
RIGHT_M = 1.5
VIEW_PITCH_DEG = -8.0
SKY_PITCH_DEG = 35.0
# The fight is filmed from the road side: behind the back the player's head hides the group.
SIDE_YAW_DEG = -40.0
SIDE_PITCH_DEG = -16.0


def hq_group(game):
    return [m for m in members(game) if m["gang"] == 0 and m["post"] == 0 and m["state"] != "Dead"]


def centroid(group):
    n = len(group)
    return [sum(m["position"][i] for m in group) / n for i in range(3)]


def yaw_to(src, dst):
    """Orbit camera yaw looking from `src` to `dst` (yaw 0 looks along -Z)."""
    return math.degrees(math.atan2(-(dst[0] - src[0]), -(dst[2] - src[2])))


def prepare(game):
    gun = pistol_pickups(game)[0]
    stand(game, gun, settle=0.5)
    me = player(game)
    game.mutate_component(me["entity"], game.component_path("Health"), ".armor", FIGHT_ARMOR)
    # Groups spawn out of view: stand on the nearest ring post with the camera turned away from the HQ.
    inner, outer = spawn_ring()
    posts = gang_posts(game, 0)
    hq = posts[0]
    approach = min((p for p in posts[1:] if inner <= horizontal(p, hq) <= outer), key=lambda p: horizontal(p, hq))
    stand(game, approach, settle=0.2)
    look(game, yaw_to(hq, approach), VIEW_PITCH_DEG)
    deadline = time.monotonic() + GROUP_TIMEOUT_S
    while not hq_group(game):
        if time.monotonic() > deadline:
            raise RuntimeError("no gang-0 HQ group")
        time.sleep(0.1)
    time.sleep(0.5)
    mid = centroid(hq_group(game))
    d = [approach[0] - mid[0], approach[2] - mid[2]]
    length = math.hypot(*d) or 1.0
    ux, uz = d[0] / length, d[1] / length
    # Looking along (-ux, -uz), the right-hand side is (uz, -ux).
    start = [mid[0] + ux * START_M + uz * RIGHT_M, hq[1], mid[2] + uz * START_M - ux * RIGHT_M]
    stand(game, start, settle=0.6)
    look(game, yaw_to(start, mid), VIEW_PITCH_DEG)
    print(f"HQ group of {len(hq_group(game))}, start {START_M} m away")
    time.sleep(0.8)


def spent(start, now):
    return sum(start[e]["rounds"] - now[e]["rounds"] for e in now if e in start and now[e]["state"] != "Dead")


def play(game):
    game.send_keys(["KeyW"], RUN_MS)
    time.sleep(RUN_MS / 1000.0 + 0.3)
    yaw = math.degrees(orbit(game)[2]["yaw"])
    pan(game, yaw, SKY_PITCH_DEG, 0.3)
    ids = {m["entity"] for m in hq_group(game)}
    start = fighters(game, ids)
    game.send_mouse_button("Left", 80)
    time.sleep(0.3)
    pan(game, yaw, VIEW_PITCH_DEG, 0.4)
    print("states after the shot:", [m["state"] for m in hq_group(game)])
    game.send_keys(["KeyW"], PUSH_MS)
    pan(game, yaw + SIDE_YAW_DEG, SIDE_PITCH_DEG, 2.5)
    pan(game, yaw + SIDE_YAW_DEG * 0.6, SIDE_PITCH_DEG, 3.5)
    now = fighters(game, ids)
    print("states at the end:", [m["state"] for m in now.values()], "rounds fired:", spent(start, now))
