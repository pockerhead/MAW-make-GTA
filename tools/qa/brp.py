"""Small JSON-RPC driver for a locally running Bevy dev build."""

import json
import math
import os
from pathlib import Path
import subprocess
import sys
import time
from urllib import request


REPO = Path(__file__).resolve().parents[2]
# The game-side release timer runs on game time, which can lag the wall clock after a hitch.
KEY_RELEASE_MARGIN_S = 0.25
# QA never reads or writes the owner's settings (%LOCALAPPDATA%\com.github.pockerhead.maw-make-gta).
QA_SETTINGS_ID = "com.github.pockerhead.maw-make-gta.qa"


class Game:
    def __init__(self, features=("dev",), args=(), port=15702, release=False):
        self.features = features
        self.release = release
        self.args = args
        self.port = port
        self.process = None
        self.log_file = None
        self.held_until = {}

    def __enter__(self):
        self.start()
        try:
            self.wait_ready()
        except Exception:
            self.stop()
            raise
        return self

    def __exit__(self, _type, _value, _traceback):
        self.stop()

    def start(self):
        command = ["cargo", "build", "-p", "gta_like", "--bin", "gta_like", "-j", "4", "--features", ",".join(self.features)]
        if self.release:
            command.append("--release")
        subprocess.run(command, cwd=REPO, check=True, timeout=1800)
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=REPO,
        ))
        executable = Path(metadata["target_directory"]) / ("release" if self.release else "debug") / ("gta_like.exe" if os.name == "nt" else "gta_like")
        log_path = REPO / "target" / "qa" / "game.log"
        log_path.parent.mkdir(parents=True, exist_ok=True)
        self.log_file = log_path.open("wb")
        env = os.environ.copy()
        env["BEVY_ASSET_ROOT"] = str(REPO)
        env["BRP_EXTRAS_PORT"] = str(self.port)
        self.process = subprocess.Popen(
            [str(executable), "--settings-id", QA_SETTINGS_ID, *self.args], cwd=REPO, env=env,
            stdout=self.log_file, stderr=subprocess.STDOUT,
        )

    def wait_ready(self, timeout=120):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                log = (REPO / "target" / "qa" / "game.log").read_text(errors="replace")[-4000:]
                raise RuntimeError(f"game exited before BRP was ready:\n{log}")
            try:
                self.call("rpc.discover", timeout=2)
                return
            except Exception:
                time.sleep(0.5)
        raise TimeoutError("BRP port did not become ready")

    def call(self, method, params=None, timeout=10):
        payload = {"jsonrpc": "2.0", "id": 1, "method": method}
        if params is not None:
            payload["params"] = params
        body = json.dumps(payload).encode("utf-8")
        req = request.Request(
            f"http://127.0.0.1:{self.port}/", data=body,
            headers={"Content-Type": "application/json"}, method="POST",
        )
        with request.urlopen(req, timeout=timeout) as response:
            data = json.load(response)
        if "error" in data:
            raise RuntimeError(f"{method}: {data['error']}")
        return data.get("result")

    def component_path(self, suffix):
        matches = [name for name in self.call("world.list_components") if name.endswith("::" + suffix)]
        if len(matches) != 1:
            raise RuntimeError(f"{suffix} not reflected/registered uniquely: {matches}")
        return matches[0]

    def resource_path(self, suffix):
        matches = [name for name in self.call("world.list_resources") if name.endswith("::" + suffix)]
        if len(matches) != 1:
            raise RuntimeError(f"{suffix} not reflected/registered uniquely as a resource: {matches}")
        return matches[0]

    def resource(self, suffix):
        value = self.call("world.get_resources", {"resource": self.resource_path(suffix)})["value"]
        if isinstance(value, list) and len(value) == 1:
            value = value[0]
        return int(value)

    def wait_resource(self, suffix, timeout):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise RuntimeError(f"game exited while waiting for {suffix}:\n{self.log_tail()}")
            try:
                return self.resource(suffix)
            except RuntimeError:
                time.sleep(0.5)
        raise TimeoutError(f"resource {suffix} did not appear in {timeout} s:\n{self.log_tail()}")

    def log_tail(self, size=4000):
        return (REPO / "target" / "qa" / "game.log").read_text(errors="replace")[-size:]

    def mutate_component(self, entity, component, path, value):
        """Verified form for avian Position (newtype over Vec3): path "" and value [x, y, z]."""
        return self.call("world.mutate_components", {
            "entity": entity, "component": component, "path": path, "value": value,
        })

    def query(self, components, with_=()):
        return self.call("world.query", {
            "data": {"components": list(components)},
            "filter": {"with": list(with_)},
            "strict": True,
        })

    def send_keys(self, keys, ms):
        """Press `keys` for `ms` ms. bevy_brp_extras releases every call on its own timer, so a key
        pressed again before its previous hold ends is released by the older timer right after the new
        press; such overlapping holds are refused. Hold once for longer (max 60000 ms) instead."""
        now = time.monotonic()
        busy = [key for key in keys if self.held_until.get(key, 0.0) > now]
        if busy:
            raise RuntimeError(
                f"send_keys: {busy} still held by an earlier call (or unknown after a failed RPC); "
                "its release would cut this press"
            )
        try:
            result = self.call("brp_extras/send_keys", {"keys": keys, "duration_ms": ms})
        except RuntimeError:
            raise  # the game answered with an error: nothing was pressed
        except Exception:
            # Transport failure: the press may still reach the game, its release time is unknowable.
            for key in keys:
                self.held_until[key] = math.inf
            raise
        # The game starts its release timer no later than its reply, so count the hold from here.
        released_by = time.monotonic() + ms / 1000.0 + KEY_RELEASE_MARGIN_S
        for key in keys:
            self.held_until[key] = released_by
        return result

    def type_text(self, text):
        """Types `text` one key per frame into the focused text field (no hold timer)."""
        return self.call("brp_extras/type_text", {"text": text})

    def move_mouse(self, dx, dy):
        return self.call("brp_extras/move_mouse", {"delta": [dx, dy]})

    def send_mouse_button(self, button, ms=100):
        """Press "Left" / "Right" for `ms` milliseconds (bevy_brp_extras releases it)."""
        return self.call("brp_extras/send_mouse_button", {"button": button, "duration_ms": ms})

    def screenshot(self, path):
        return self.call("brp_extras/screenshot", {"path": str(Path(path).resolve())}, timeout=60)

    def diagnostics(self):
        return self.call("brp_extras/get_diagnostics")

    def window_state(self):
        window = self.component_path("Window")
        rows = self.query([window], with_=[window])
        if len(rows) != 1:
            raise RuntimeError(f"expected one game window; got {len(rows)}")
        value = rows[0]["components"][window]
        return {"focused": value["focused"], "present_mode": value["present_mode"]}

    def frame_report(self, samples=3):
        """FPS under vsync is the display refresh, not the frame cost (TASK-002: a 30 Hz virtual display
        read exactly 30 FPS). Records monitors and present mode, samples FPS as shipped, then switches the
        window to `AutoNoVsync` (it stays there) and samples the frame cost."""
        monitor, primary = self.component_path("Monitor"), self.component_path("PrimaryMonitor")
        primaries = {row["entity"] for row in self.query([], with_=[primary])}
        monitors = [
            {"name": row["components"][monitor]["name"],
             "refresh_hz": (row["components"][monitor]["refresh_rate_millihertz"] or 0) / 1000.0,
             "primary": row["entity"] in primaries}
            for row in self.query([monitor])
        ]

        def sample():
            out = []
            for _ in range(samples):
                time.sleep(1.0)
                d = self.diagnostics()
                out.append({"fps": round(d["fps"]["average"], 1), "frame_ms": round(d["frame_time_ms"]["average"], 3)})
            return out

        shipped = self.window_state()["present_mode"]
        as_shipped = sample()
        window = self.component_path("Window")
        entity = self.query([window], with_=[window])[0]["entity"]
        self.mutate_component(entity, window, ".present_mode", "AutoNoVsync")
        time.sleep(1.0)
        cost = sample()
        return {
            "monitors": monitors,
            "present_mode": shipped,
            "fps_as_shipped": as_shipped,
            "frame_cost_no_vsync": cost,
            "frame_cost_ms_worst_avg": max(s["frame_ms"] for s in cost),
        }

    def shutdown(self):
        return self.call("brp_extras/shutdown")

    def stop(self):
        if self.process is None:
            return
        try:
            if self.process.poll() is None:
                try:
                    self.shutdown()
                    self.process.wait(timeout=15)
                except Exception:
                    self.process.kill()
                    self.process.wait(timeout=15)
        finally:
            self.process = None
            if self.log_file is not None:
                self.log_file.close()
                self.log_file = None


def load_golden():
    """Golden layout hashes {seed: hash} from citygen's golden_hashes.txt (same rule as the Rust gates)."""
    golden = {}
    text = (REPO / "crates" / "citygen" / "tests" / "golden_hashes.txt").read_text(encoding="utf-8")
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        seed, value = line.split(" ", 1)
        golden[int(seed)] = int(value.strip(), 16)
    return golden


def vec3(value):
    if isinstance(value, dict):
        return value["x"], value["y"], value["z"]
    return tuple(value)


if __name__ == "__main__":
    with Game() as game:
        print(game.diagnostics())
