"""Check the headless boundary and pinned render dependency versions."""

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]


def tree(*args):
    result = subprocess.run(["cargo", "tree", *args], capture_output=True, text=True, cwd=REPO)
    if result.returncode:
        raise RuntimeError(f"cargo tree {' '.join(args)} failed:\n{result.stderr}")
    return result.stdout


def packages(output):
    return re.findall(r"^([A-Za-z0-9_-]+) v([0-9][^\s]*)", output, re.M)


def check():
    for edges in ("normal", "normal,dev"):
        output = tree("-p", "gta_sim", "-e", edges, "-i", "bevy_render")
        if packages(output):
            raise RuntimeError(f"bevy_render leaks into gta_sim ({edges}):\n{output}")

    image = packages(tree("-i", "image", "--features", "dev,debug", "-e", "normal", "--depth", "0"))
    if image != [("image", "0.25.9")]:
        raise RuntimeError(f"image version drift: {image}")

    egui = packages(tree("-i", "bevy_egui", "--features", "dev,debug", "-e", "normal", "--depth", "0"))
    if len(egui) != 1 or egui[0][0] != "bevy_egui" or not egui[0][1].startswith("0.40."):
        raise RuntimeError(f"bevy_egui version drift: {egui}")

    duplicates = packages(tree("-d", "--features", "dev,debug", "-e", "normal", "--depth", "0"))
    critical = re.compile(r"^(?:bevy(?:[_-].*)?|avian3d|parry3d|egui|image|wgpu.*|naga|winit)$")
    bad = [name for name, _ in duplicates if critical.fullmatch(name)]
    if bad:
        raise RuntimeError(f"critical duplicate packages: {bad}")
    print(f"tree checks passed; unrelated duplicate packages: {duplicates}")


if __name__ == "__main__":
    try:
        check()
    except RuntimeError as error:
        print(error, file=sys.stderr)
        sys.exit(1)
