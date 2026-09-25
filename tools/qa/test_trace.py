"""Offline gate for `trace.summarize`: a synthetic chrome trace, no game runs.

Run: python -m unittest tools/qa/test_trace.py
"""

import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import trace  # noqa: E402

MODULES = {"gang", "police", "vehicle", "tactics"}


def b(tid, ts, name):
    return {"ph": "B", "pid": 1, "tid": tid, "ts": ts, "name": name, "cat": "bevy"}


def e(tid, ts, name):
    return {"ph": "E", "pid": 1, "tid": tid, "ts": ts, "name": name, "cat": "bevy"}


def frame(t0):
    """One 10 ms frame at `t0` µs on thread 1 with a 4 ms FixedMain tick; on thread 2 a gang system
    (2 ms) with a vehicle system nested in it (0.5 ms: gang's own time is 1.5 ms), a physics runner
    (1.2 ms) running the solver inline (1 ms), and an unknown module (0.25 ms)."""
    gang = 'system: name="gta_sim::gang::behavior::drive_members"'
    car = 'system: name="gta_sim::vehicle::chassis::drive"'
    runner = 'system: name="avian3d::schedule::run_physics_schedule"'
    physics = 'system: name="avian3d::solver::solve"'
    odd = 'system: name="gta_sim::weather::rain"'
    return [
        b(1, t0, "update"),
        b(1, t0 + 1000, "schedule: name=FixedMain"),
        b(2, t0 + 1000, gang),
        b(2, t0 + 1500, car),
        e(2, t0 + 2000, car),
        e(2, t0 + 3000, gang),
        b(2, t0 + 3000, runner),
        b(2, t0 + 3100, physics),
        e(2, t0 + 4100, physics),
        e(2, t0 + 4200, runner),
        b(2, t0 + 4200, odd),
        e(2, t0 + 4450, odd),
        e(1, t0 + 5000, "schedule: name=FixedMain"),
        e(1, t0 + 10000, "update"),
    ]


class Summarize(unittest.TestCase):
    def write(self, events, tail):
        handle = tempfile.NamedTemporaryFile("w", suffix=".json", delete=False, encoding="utf-8")
        self.addCleanup(lambda: Path(handle.name).unlink())
        handle.write("[\n" + ",\n".join(json.dumps(ev) for ev in events) + tail)
        handle.close()
        return handle.name

    def test_three_frames_with_a_truncated_last_line(self):
        events = frame(0.0) + frame(20000.0) + frame(40000.0)
        path = self.write(events, ',\n{"ph":"B","pid":1,"tid":1,"ts":60000.0,"na')
        s = trace.summarize(path, window_s=10.0, modules=MODULES)
        self.assertEqual(s["frames"]["count"], 3)
        self.assertAlmostEqual(s["frames"]["mean_ms"], 10.0)
        self.assertEqual(s["fixed_main"]["count"], 3)
        self.assertAlmostEqual(s["fixed_main"]["mean_ms"], 4.0)
        totals = s["systems_ms_total"]
        # Exclusive times: the nested vehicle system is out of gang's, the solver out of its runner's.
        self.assertAlmostEqual(totals["gta_sim::gang::behavior::drive_members"], 4.5)
        self.assertAlmostEqual(totals["gta_sim::vehicle::chassis::drive"], 1.5)
        self.assertAlmostEqual(totals["avian3d::solver::solve"], 3.0)
        self.assertAlmostEqual(totals["avian3d::schedule::run_physics_schedule"], 0.6)
        self.assertEqual(s["systems_ms_per_frame"][0][0], "gta_sim::gang::behavior::drive_members")
        self.assertAlmostEqual(s["systems_ms_per_frame"][0][1], 1.5)
        self.assertAlmostEqual(s["ms_per_tick"]["ai"], 1.5)
        # The runner's own time is waiting, not physics.
        self.assertAlmostEqual(s["ms_per_tick"]["physics"], 1.0)
        self.assertAlmostEqual(s["ms_per_tick"]["unclassified"], 0.25)
        self.assertEqual(s["unclassified_modules"], ["weather"])

    def test_window_keeps_only_the_last_spans(self):
        events = frame(0.0) + frame(20000.0) + frame(40000.0)
        path = self.write(events, "\n]")
        # max ts = 50 000 µs; a 0.025 s window starts at 25 000 µs: only the third frame.
        s = trace.summarize(path, window_s=0.025, modules=MODULES)
        self.assertEqual(s["frames"]["count"], 1)
        self.assertAlmostEqual(s["systems_ms_total"]["avian3d::solver::solve"], 1.0)

    def test_bisection_reads_the_same_window_as_a_full_scan(self):
        # ~12 000 frames at 20 ms: several MB, so the reader seeks instead of starting at byte 0.
        events = [ev for k in range(12000) for ev in frame(k * 20000.0)]
        path = self.write(events, "\n]")
        self.assertGreater(Path(path).stat().st_size, 4 << 20)
        # A fine bracket, so the seek lands close to the window start and the margin matters.
        with mock.patch.object(trace, "BISECT_BYTES", 4096):
            self.assertGreater(trace.window_offset(path, trace.max_ts(path) - 10e6), 4 << 20)
            s = trace.summarize(path, window_s=10.0, modules=MODULES)
        # max ts = 239 990 ms; frames starting at >= 229 990 ms: k = 11 500 .. 11 999.
        self.assertEqual(s["frames"]["count"], 500)
        self.assertAlmostEqual(s["systems_ms_total"]["avian3d::solver::solve"], 500.0)

    def test_span_names(self):
        self.assertEqual(trace.span_kind("update"), ("update", None))
        self.assertEqual(trace.span_kind("schedule: name=FixedMain"), ("schedule", "FixedMain"))
        self.assertEqual(trace.span_kind('system: name="a::b::c"'), ("system", "a::b::c"))

    def test_modules_come_from_lib_rs(self):
        modules = trace.sim_modules()
        self.assertIn("gang", modules)
        self.assertIn("tactics", modules)


if __name__ == "__main__":
    unittest.main()
