"""City: a sprint down the street from the spawn point, then the view from the edge of the 156 m tower roof."""

import time

from _common import landmark, look, pan, stand

CAPTION = "Процедурный город: улицы, парк, пропы Kenney и вид с крыши башни 156 м"
ANCHOR = "- TASK-004 (T3)"
SEED = 1
ROOF_EDGE_M = 4.5


def prepare(game):
    look(game, 0.0, -6.0)
    time.sleep(1.0)


def play(game):
    game.send_keys(["KeyW", "ShiftLeft"], 4000)
    pan(game, 30.0, -4.0, 4.0)
    roof = landmark(game, "tower_roof")
    stand(game, [roof[0], roof[1], roof[2] + ROOF_EDGE_M], settle=0.2)
    look(game, 140.0, -42.0)
    pan(game, 215.0, -48.0, 5.0)
