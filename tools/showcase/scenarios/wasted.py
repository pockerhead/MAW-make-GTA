"""Death: a hit drains the health bar, a lethal hit gives slow motion and "ПОТРАЧЕНО", then the hospital respawn."""

import time

from _common import pan
from t5 import damage, game_state, poll

CAPTION = "Смерть: замедление, экран \"ПОТРАЧЕНО\" и возрождение у больницы"
ANCHOR = "- TASK-006 (T5)"
SEED = 1


def prepare(game):
    time.sleep(0.5)


def play(game):
    game.send_keys(["KeyW"], 1800)
    time.sleep(1.0)
    damage(game, 40.0)
    time.sleep(1.0)
    damage(game, 1000.0)
    pan(game, 60.0, -20.0, 1.5)
    poll("respawn", lambda: game_state(game) == "Playing", 10.0, interval=0.1)
    time.sleep(1.8)
