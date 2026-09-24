"""Record short gameplay GIFs of the real windowed game, one per scenario in tools/showcase/scenarios/.

Usage:
  python tools/showcase/record.py --all              every scenario
  python tools/showcase/record.py city shooting      the named scenarios
  python tools/showcase/record.py --all --no-build   reuse the last release dev build

The release dev build is copied to a temp dir and the copy runs, with a unique window title and its
own BRP port, so a second game window or a running build of another checkout never interferes.
ffmpeg grabs that window by title (gdigrab), the scenario drives the game over BRP, then the capture
becomes an optimized GIF in <cargo target dir>/showcase/.
"""

import argparse
import ctypes
import ctypes.wintypes
import importlib.util
import json
import os
import re
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time

REPO = Path(__file__).resolve().parents[2]
SCENARIOS = Path(__file__).resolve().parent / "scenarios"
sys.path.insert(0, str(REPO / "tools" / "qa"))
sys.path.insert(0, str(REPO / "tools" / "qa" / "scenarios"))
sys.path.insert(0, str(SCENARIOS))
from brp import Game  # noqa: E402

FFMPEG = shutil.which("ffmpeg") or "C:/ProgramData/chocolatey/bin/ffmpeg.exe"
CAPTURE_FPS = 30
MAX_SECONDS = 12
MAX_BYTES = 5 * 1024 * 1024
# (width px, fps, palette colours), tried in order until the GIF fits MAX_BYTES.
GIF_PRESETS = ((720, 15, 256), (640, 15, 192), (640, 12, 128), (640, 10, 128), (560, 12, 96), (480, 10, 64))
EXE = "gta_like.exe" if os.name == "nt" else "gta_like"


def target_dir():
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=REPO,
    ))
    return Path(metadata["target_directory"])


def build():
    subprocess.run(
        ["cargo", "build", "--release", "-p", "gta_like", "--bin", "gta_like", "-j", "4", "--features", "dev"],
        cwd=REPO, check=True, timeout=1800,
    )


def free_port():
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


def load_scenarios(names):
    unknown = sorted(set(names) - set(scenario_names()))
    if unknown:
        raise SystemExit(f"unknown scenario(s) {unknown}; available: {scenario_names()}")
    modules = []
    for name in names:
        spec = importlib.util.spec_from_file_location(f"showcase_{name}", SCENARIOS / f"{name}.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        module.NAME = name
        modules.append(module)
    return modules


def scenario_names():
    return [p.stem for p in sorted(SCENARIOS.glob("*.py")) if not p.stem.startswith("_")]


class ShowcaseGame(Game):
    """`brp.Game` that runs a copied exe with its own title, port and log instead of building."""

    def __init__(self, exe, seed, title, log_path):
        super().__init__(port=free_port())
        self.exe = exe
        self.seed = seed
        self.title = title
        self.log_path = log_path

    def start(self):
        self.log_file = self.log_path.open("wb")
        env = os.environ.copy()
        env["BEVY_ASSET_ROOT"] = str(REPO)
        env["BRP_EXTRAS_PORT"] = str(self.port)
        self.process = subprocess.Popen(
            [str(self.exe), "--seed", str(self.seed), "--window-title", self.title,
             "--settings-id", "com.github.pockerhead.maw-make-gta.qa"],
            cwd=self.exe.parent, env=env, stdout=self.log_file, stderr=subprocess.STDOUT,
        )

    def wait_ready(self, timeout=120):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise RuntimeError(f"game exited before BRP was ready:\n{self.log_tail()}")
            try:
                self.call("rpc.discover", timeout=2)
                return
            except Exception:
                time.sleep(0.5)
        raise TimeoutError("BRP port did not become ready")

    def log_tail(self, size=4000):
        return self.log_path.read_text(errors="replace")[-size:]


def wait_city(game, timeout=180):
    """City generated and its chunk meshes spawned (two equal non-zero counts a second apart)."""
    game.wait_resource("CityLayoutHash", timeout)
    chunk = game.component_path("CityChunk")
    deadline = time.monotonic() + timeout
    last = -1
    while time.monotonic() < deadline:
        count = len(game.query([chunk], with_=[chunk]))
        if count > 0 and count == last:
            return count
        last = count
        time.sleep(1.0)
    raise TimeoutError(f"city chunks did not settle in {timeout} s (last {last})")


def raise_window(title):
    """Bring the game window to the top and return its client rect (x, y, w, h) in screen pixels.

    gdigrab `title=` reads the window DC, which a GPU swapchain never paints (it returns a stale
    frame), so the capture grabs this region of the composed desktop and the window must be on top."""
    user32 = ctypes.windll.user32
    HWND = ctypes.wintypes.HWND
    user32.FindWindowW.restype = HWND
    user32.SetWindowPos.argtypes = [HWND, HWND] + [ctypes.c_int] * 4 + [ctypes.c_uint]
    user32.SetProcessDPIAware()
    hwnd = user32.FindWindowW(None, title)
    if not hwnd:
        raise RuntimeError(f"no window titled {title!r}")
    topmost, flags = HWND(-1), 0x0001 | 0x0002 | 0x0040  # HWND_TOPMOST; SWP_NOSIZE | SWP_NOMOVE | SWP_SHOWWINDOW
    if not user32.SetWindowPos(hwnd, topmost, 0, 0, 0, 0, flags):
        raise ctypes.WinError()
    rect = ctypes.wintypes.RECT()
    user32.GetClientRect(HWND(hwnd), ctypes.byref(rect))
    origin = ctypes.wintypes.POINT(0, 0)
    user32.ClientToScreen(HWND(hwnd), ctypes.byref(origin))
    width, height = rect.right - rect.left, rect.bottom - rect.top
    return origin.x, origin.y, width - width % 2, height - height % 2


class Capture:
    """ffmpeg gdigrab of the game window's screen region, stopped gracefully with "q" on stdin."""

    def __init__(self, title, path, log_path):
        self.log = log_path.open("wb")
        x, y, width, height = raise_window(title)
        time.sleep(0.5)
        self.process = subprocess.Popen(
            [FFMPEG, "-hide_banner", "-y", "-f", "gdigrab", "-framerate", str(CAPTURE_FPS),
             "-draw_mouse", "0", "-offset_x", str(x), "-offset_y", str(y), "-video_size", f"{width}x{height}",
             "-i", "desktop", "-t", str(MAX_SECONDS + 5),
             "-c:v", "libx264", "-preset", "ultrafast", "-crf", "16", "-pix_fmt", "yuv444p", str(path)],
            stdin=subprocess.PIPE, stdout=self.log, stderr=subprocess.STDOUT,
        )
        # The muxer writes nothing for seconds (encoder lookahead); the progress line shows real frames.
        deadline = time.monotonic() + 10
        while not re.search(rb"frame=\s*[1-9]", log_path.read_bytes()):
            if self.process.poll() is not None or time.monotonic() > deadline:
                self.stop()
                raise RuntimeError(f"ffmpeg did not start capturing {title!r}: see {log_path}")
            time.sleep(0.05)

    def stop(self):
        if self.process.poll() is None:
            try:
                self.process.communicate(b"q", timeout=15)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=15)
        self.log.close()


def to_gif(raw, gif):
    for width, fps, colors in GIF_PRESETS:
        graph = (f"fps={fps},scale={width}:-2:flags=lanczos,split[a][b];"
                 f"[a]palettegen=max_colors={colors}:stats_mode=diff[p];"
                 f"[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle")
        subprocess.run(
            [FFMPEG, "-hide_banner", "-loglevel", "error", "-y", "-t", str(MAX_SECONDS), "-i", str(raw),
             "-filter_complex", graph, "-loop", "0", str(gif)],
            check=True,
        )
        if gif.stat().st_size <= MAX_BYTES:
            return {"width": width, "fps": fps, "colors": colors, "bytes": gif.stat().st_size}
    raise RuntimeError(f"{gif} is still over {MAX_BYTES} bytes at the smallest preset")


def duration(path):
    out = subprocess.check_output([
        FFMPEG.replace("ffmpeg", "ffprobe"), "-v", "error", "-show_entries", "format=duration",
        "-of", "default=nw=1:nk=1", str(path),
    ])
    return float(out)


def record(scenario, exe, out):
    title = f"GTA-like showcase {scenario.NAME} {os.getpid()}"
    raw = out / f"{scenario.NAME}.mkv"
    gif = out / f"{scenario.NAME}.gif"
    with ShowcaseGame(exe, getattr(scenario, "SEED", 1), title, out / f"{scenario.NAME}.game.log") as game:
        wait_city(game)
        scenario.prepare(game)
        capture = Capture(title, raw, out / f"{scenario.NAME}.ffmpeg.log")
        try:
            started = time.monotonic()
            scenario.play(game)
            played = time.monotonic() - started
        finally:
            capture.stop()
        game.shutdown()
        game.process.wait(timeout=15)
    info = to_gif(raw, gif)
    info.update({"gif": str(gif), "played_s": round(played, 2), "gif_s": round(duration(gif), 2)})
    if info["gif_s"] > MAX_SECONDS + 0.1:
        raise RuntimeError(f"{gif} lasts {info['gif_s']} s")
    return info


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("names", nargs="*", help="scenario names (file stems in tools/showcase/scenarios)")
    parser.add_argument("--all", action="store_true", help="record every scenario")
    parser.add_argument("--no-build", action="store_true", help="skip cargo build, reuse the last release dev exe")
    parser.add_argument("--out", type=Path, help="output dir (default <cargo target dir>/showcase)")
    args = parser.parse_args()
    names = scenario_names() if args.all else args.names
    if not names:
        parser.error("name a scenario or pass --all")
    scenarios = load_scenarios(names)
    if subprocess.run([sys.executable, str(REPO / "tools" / "fetch_assets.py"), "--check"], cwd=REPO).returncode:
        raise SystemExit("run python tools/fetch_assets.py first")
    if not args.no_build:
        build()
    target = target_dir()
    out = (args.out or target / "showcase").resolve()
    out.mkdir(parents=True, exist_ok=True)
    results = {}
    with tempfile.TemporaryDirectory(prefix="gta-showcase-") as temp:
        # The copy runs, so the shared target exe is never locked by a recording.
        exe = Path(temp) / EXE
        shutil.copy2(target / "release" / EXE, exe)
        for scenario in scenarios:
            print(f"recording {scenario.NAME} ...", flush=True)
            results[scenario.NAME] = record(scenario, exe, out)
            print(json.dumps(results[scenario.NAME]), flush=True)
    (out / "record.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
    print(f"GIFs in {out}")


if __name__ == "__main__":
    main()
