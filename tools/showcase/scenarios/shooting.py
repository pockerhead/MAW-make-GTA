"""Shooting range in the central park: pistol body shots, a red CRIT headshot, then an aimed SMG burst."""

import time

from _common import stand
from t6 import CHEST_M, HEAD_M, aim_at, camera_config, damage_numbers, dummies, player, weapon_pickups, weapon_table

CAPTION = "Тир в парке: пистолет, хедшот с красным CRIT, очередь из SMG"
ANCHOR = "- TASK-007 (T6)"
SEED = 1
STAND_OFF_M = 5.0
AIM_HOLD_MS = 12000


def pick_up(game, weapon):
    item = next(i for i in weapon_pickups(game) if i["weapon"] == weapon and not i["ammo_only"])
    stand(game, item["at"], settle=0.5)


def feet(game, target):
    x, y, z = target["position"]
    return [x, y - player(game)["float_height"], z]


def prepare(game):
    pick_up(game, "Smg")
    pick_up(game, "Pistol")
    game.send_keys(["Digit2"], 100)
    middle = dummies(game)[1]
    base = feet(game, middle)
    stand(game, [base[0] + 1.5, base[1], base[2] + STAND_OFF_M], settle=1.0)
    aim_at(game, [base[0], base[1] + CHEST_M, base[2]], camera_config()["sensitivity_deg"])


def play(game):
    table = weapon_table()
    cam = camera_config()
    aim_sensitivity = cam["sensitivity_deg"] * cam["aim_scale"]
    targets = dummies(game)
    middle = feet(game, targets[1])
    time.sleep(0.6)
    game.send_mouse_button("Right", AIM_HOLD_MS)
    time.sleep(0.5)
    aim_at(game, [middle[0], middle[1] + CHEST_M, middle[2]], aim_sensitivity)
    for _ in range(2):
        game.send_mouse_button("Left", 80)
        time.sleep(table["pistol"]["fire_interval"] + 0.35)
    for _ in range(3):
        aim_at(game, [middle[0], middle[1] + HEAD_M, middle[2]], aim_sensitivity)
        game.send_mouse_button("Left", 80)
        time.sleep(0.25)
        if any(n["headshot"] for n in damage_numbers(game)):
            break
        time.sleep(table["pistol"]["fire_interval"])
    time.sleep(0.8)
    game.send_keys(["Digit3"], 100)
    side = feet(game, targets[2])
    time.sleep(0.4)
    aim_at(game, [side[0], side[1] + CHEST_M, side[2]], aim_sensitivity)
    game.send_mouse_button("Left", 1500)
    time.sleep(2.2)
