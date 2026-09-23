"""Melee on the park range: a three-punch combo (jab, jab, knockdown kick), then a bat swing knocks a dummy down."""

import time

from brp import vec3
from _common import stand, tilt
from t6 import camera_config, dummies, rows
from t7 import face_dummy, me

CAPTION = "Ближний бой: комбо кулаков с нокдауном и удар битой"
ANCHOR = "- TASK-008 (T7)"
SEED = 1
CLICK_MS = 80
COMBO_GAP_S = 0.3
# Looking down over the player's head: from behind at eye level the player hides the dummy.
PITCH_DEG = -30.0


def prepare(game):
    bat = [vec3(t["translation"]) for _, (_, t) in rows(game, ["BatPickup", "Transform"])][0]
    stand(game, bat, settle=0.5)
    game.send_keys(["Digit1"], 100)
    time.sleep(0.4)
    if me(game)["melee"] != "Fists":
        raise RuntimeError(f"expected fists after the bat pickup toggle, got {me(game)['melee']}")
    face_dummy(game, dummies(game)[1], camera_config()["sensitivity_deg"])
    tilt(game, PITCH_DEG)


def play(game):
    time.sleep(0.6)
    for _ in range(3):
        game.send_mouse_button("Left", CLICK_MS)
        time.sleep(COMBO_GAP_S)
    time.sleep(2.2)
    game.send_keys(["Digit1"], 100)
    face_dummy(game, dummies(game)[0], camera_config()["sensitivity_deg"])
    tilt(game, PITCH_DEG)
    time.sleep(0.3)
    game.send_mouse_button("Left", CLICK_MS)
    time.sleep(2.5)
