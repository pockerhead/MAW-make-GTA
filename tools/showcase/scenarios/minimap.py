"""Minimap: a run with the camera turning (the map turns with it), two stars over BRP (search circle, cop cones),
then Esc, a typed seed and Enter build another city."""

import time

from _common import look, pan
from t5 import game_state
from t6 import player
from t11 import ARMOR, set_heat
from t12 import pause_menu

CAPTION = "Мини-карта вращается с камерой, при розыске на ней круг поиска и конусы копов; Esc, новый seed — другой город"
ANCHOR = "- TASK-013 (T12)"
SEED = 1
NEW_SEED = "42"
HEAT = 200  # two stars by wanted.ron: a 70 m search circle and four patrol units
RUN_MS = 5000
VIEW_PITCH_DEG = -8.0
TURN_DEG = 50.0
TYPE_WAIT_S = 0.5
MENU_HOLD_S = 1.0


def wait_for(what, predicate, timeout):
    deadline = time.monotonic() + timeout
    while not predicate():
        if time.monotonic() > deadline:
            raise TimeoutError(what)
        time.sleep(0.02)


def state_is(game, name):
    try:
        return game_state(game) == name
    except RuntimeError:
        return False


def prepare(game):
    me = player(game)
    # Two-star cops shoot; the clip is about the map, not about dying.
    game.mutate_component(me["entity"], game.component_path("Health"), ".armor", ARMOR)
    look(game, 0.0, VIEW_PITCH_DEG)
    time.sleep(1.0)


def play(game):
    set_heat(game, HEAT)
    game.send_keys(["KeyW", "ShiftLeft"], RUN_MS)
    started = time.monotonic()
    pan(game, TURN_DEG, VIEW_PITCH_DEG, RUN_MS / 2000.0)
    pan(game, -TURN_DEG * 0.4, VIEW_PITCH_DEG, RUN_MS / 2000.0)
    # The hold is released on the virtual clock, which stops while paused: let it end first.
    time.sleep(max(0.0, RUN_MS / 1000.0 + 0.15 - (time.monotonic() - started)))
    game.send_keys(["Escape"], 100)
    wait_for("PauseMenu Main", lambda: pause_menu(game) == "Main", 5)
    time.sleep(MENU_HOLD_S)
    game.type_text(NEW_SEED)
    time.sleep(TYPE_WAIT_S)
    game.send_keys(["Enter"], 100)
    wait_for("left Paused", lambda: not state_is(game, "Paused"), 10)
    loading = time.monotonic()
    wait_for("Playing in the new city", lambda: state_is(game, "Playing"), 60)
    print(f"new city in {time.monotonic() - loading:.1f} s")
    look(game, 0.0, VIEW_PITCH_DEG)
    pan(game, 60.0, VIEW_PITCH_DEG - 4.0, 2.0)
