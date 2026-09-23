"""Offline gate for the `Game.send_keys` overlap guard: `call` and the clock are stubbed, no game runs.

Run: python -m unittest tools/qa/test_brp.py
"""

from pathlib import Path
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import brp  # noqa: E402

MARGIN = brp.KEY_RELEASE_MARGIN_S


class FakeClock:
    def __init__(self):
        self.now = 100.0

    def __call__(self):
        return self.now


class SendKeysGuard(unittest.TestCase):
    def setUp(self):
        self.clock = FakeClock()
        patcher = mock.patch.object(brp.time, "monotonic", self.clock)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.game = brp.Game()
        self.sent = []
        self.game.call = self.ok_call

    def ok_call(self, method, params=None, timeout=10):
        self.sent.append((method, params["keys"], params["duration_ms"]))
        return "ok"

    def test_press_is_sent_and_returns_result(self):
        self.assertEqual(self.game.send_keys(["KeyW"], 5000), "ok")
        self.assertEqual(self.sent, [("brp_extras/send_keys", ["KeyW"], 5000)])

    def test_overlapping_press_is_refused_before_rpc(self):
        # TASK-022 pattern: a 5000 ms hold re-sent every 4.8 s.
        self.game.send_keys(["KeyW"], 5000)
        self.clock.now += 4.8
        with self.assertRaises(RuntimeError):
            self.game.send_keys(["KeyW"], 5000)
        self.assertEqual(len(self.sent), 1)

    def test_other_key_is_independent(self):
        self.game.send_keys(["KeyW"], 5000)
        self.game.send_keys(["KeyA"], 100)
        self.assertEqual([keys for _, keys, _ in self.sent], [["KeyW"], ["KeyA"]])

    def test_press_after_hold_and_margin_is_accepted(self):
        self.game.send_keys(["KeyW"], 300)
        self.clock.now += 0.3 + MARGIN + 0.01
        self.game.send_keys(["KeyW"], 100)
        self.assertEqual(len(self.sent), 2)

    def test_hold_counts_from_delayed_response(self):
        def slow_call(method, params=None, timeout=10):
            self.clock.now += 0.4  # the reply arrives after the 100 ms hold would have ended locally
            return self.ok_call(method, params, timeout)

        self.game.call = slow_call
        self.game.send_keys(["KeyW"], 100)
        self.clock.now += 0.1 + MARGIN - 0.01
        with self.assertRaises(RuntimeError):
            self.game.send_keys(["KeyW"], 100)
        self.clock.now += 0.02
        self.game.send_keys(["KeyW"], 100)
        self.assertEqual(len(self.sent), 2)

    def test_rpc_error_reply_does_not_reserve_the_key(self):
        def error_call(method, params=None, timeout=10):
            raise RuntimeError(f"{method}: rejected")

        self.game.call = error_call
        with self.assertRaises(RuntimeError):
            self.game.send_keys(["KeyW"], 5000)
        self.game.call = self.ok_call
        self.game.send_keys(["KeyW"], 100)
        self.assertEqual(len(self.sent), 1)

    def test_transport_failure_blocks_the_key(self):
        # A timed-out press may still land in the game with a release timer we cannot see.
        def timeout_call(method, params=None, timeout=10):
            raise TimeoutError("timed out")

        self.game.call = timeout_call
        with self.assertRaises(TimeoutError):
            self.game.send_keys(["KeyW"], 100)
        self.game.call = self.ok_call
        self.clock.now += 3600.0
        with self.assertRaises(RuntimeError):
            self.game.send_keys(["KeyW"], 100)
        self.game.send_keys(["KeyA"], 100)
        self.assertEqual([keys for _, keys, _ in self.sent], [["KeyA"]])


if __name__ == "__main__":
    unittest.main()
