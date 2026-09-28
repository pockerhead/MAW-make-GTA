"""Release zip packaging and checks (TASK-028, docs/decisions/ADR-002-release-ci.md).

    python tools/package_release.py package --exe PATH --name NAME --out DIR
        Zip NAME/<exe> + NAME/assets/** (tracked files + third-party packs of the manifest) into DIR/NAME.zip,
        then run `verify` on it. Refuses an exe that links Rust/Bevy or the MSVC CRT dynamically.
    python tools/package_release.py verify ZIP
        Allowlist (exe + assets only), sha256 per asset (manifest for packs, checkout for tracked files),
        licence per pack, no dynamic Rust/Bevy/CRT imports, GUI subsystem (no console) on the Windows exe,
        exec bit on the Linux exe.
    python tools/package_release.py smoke DIR --seconds N --log FILE [--expect TEXT]... [--allow REGEX]... [-- GAME_ARGS...]
        Boot the unpacked DIR/gta_like[.exe] from a temp cwd without CARGO_MANIFEST_DIR/BEVY_ASSET_ROOT, keep it
        alive N seconds, copy the game's own log DIR/gta_like.log to FILE (console output goes to the
        FILE-stem.console.log sibling), then require every --expect text and zero ERROR/panic lines not matched
        by an --allow in that log.

A product failure prints "<subcommand>: <check>: ..." and exits 1; a missing input prints
"<subcommand>: GATE BROKEN: ..." and exits 2.
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from fetch_assets import MANIFEST, ManifestError, check, load_manifest, sha256_bytes, sha256_file  # noqa: E402

REPO = Path(__file__).resolve().parents[1]
EXE_NAMES = ("gta_like.exe", "gta_like")
DYLIB = re.compile(rb"(?:lib)?(?:bevy_dylib|std-[0-9a-f]{16})\.(?:dll|so)")
# Anchored on api-ms-win-crt-: a +crt-static exe still carries other api-ms-win-* names (scratch/crt_static_probe).
CRT = re.compile(
    rb"(?i)(?:vcruntime140(?:_1|_threads)?|msvcp140(?:_[0-9a-z_]+)?|ucrtbased?|api-ms-win-crt-[a-z0-9-]+)\.dll")
GAME_LOG = "gta_like.log"
PE_GUI_SUBSYSTEM = 2
ANSI = re.compile(r"\x1b\[[0-9;]*m")
BAD_LINE = re.compile(r"\bERROR\b|panicked")
POLL_SECONDS = 0.5
TAIL_LINES = 40


class GateBroken(Exception):
    """An input the gate needs is missing: the gate's plumbing failed, not the product."""


def dynamic_imports(data, windows):
    problems = []
    dylibs = sorted({m.decode() for m in DYLIB.findall(data)})
    if dylibs:
        problems.append(f"exe links Rust/Bevy dynamically: {', '.join(dylibs)} (built with `fast`?)")
    crt = sorted({m.decode() for m in CRT.findall(data)}) if windows else []
    if crt:
        problems.append(f"exe links the MSVC CRT dynamically: {', '.join(crt)} (built without +crt-static)")
    return problems


def pe_subsystem(data):
    """IMAGE_OPTIONAL_HEADER.Subsystem of a PE image (2 = GUI, 3 = console); None if `data` is not a PE."""
    if len(data) < 0x40 or data[:2] != b"MZ":
        return None
    pe = int.from_bytes(data[0x3C:0x40], "little")
    field = pe + 24 + 68
    if data[pe:pe + 4] != b"PE\0\0" or len(data) < field + 2:
        return None
    return int.from_bytes(data[field:field + 2], "little")


def exe_problems(data, windows):
    problems = dynamic_imports(data, windows)
    if not windows:
        return problems
    subsystem = pe_subsystem(data)
    if subsystem != PE_GUI_SUBSYSTEM:
        problems.append(f"exe subsystem {subsystem}, expected {PE_GUI_SUBSYSTEM} (GUI): "
                        "a console window opens with the game (built without windows_subsystem?)")
    return problems


def manifest():
    try:
        return load_manifest(MANIFEST)
    except (OSError, ManifestError) as error:
        raise GateBroken(f"manifest {MANIFEST} unreadable: {error}") from error


def expected_assets(doc):
    """Paths relative to assets/: tracked files plus every file of every third-party pack."""
    try:
        out = subprocess.run(["git", "-C", str(REPO), "ls-files", "-z", "assets"],
                             capture_output=True, check=True).stdout
    except (OSError, subprocess.CalledProcessError) as error:
        raise GateBroken(f"git ls-files assets failed: {error}") from error
    tracked = {p.removeprefix("assets/") for p in out.decode().split("\0") if p}
    packs = {f"third_party/{pack['name']}/{f['path']}" for pack in doc["packs"] for f in pack["files"]}
    return sorted(tracked | packs)


def pack_shas(doc):
    return {f"third_party/{pack['name']}/{f['path']}": f["sha256"] for pack in doc["packs"] for f in pack["files"]}


def expected_sha(rel, shas):
    """Manifest sha for pack files (fixed, independent of the local checkout); checkout sha for tracked files."""
    if rel in shas:
        return shas[rel]
    path = REPO / "assets" / rel
    if not path.is_file():
        raise GateBroken(f"tracked file {path} is missing from the checkout")
    return sha256_file(path)


def entry(name, source, mode):
    info = zipfile.ZipInfo(name, time.localtime(source.stat().st_mtime)[:6])
    info.create_system = 3
    info.external_attr = (0o100000 | mode) << 16
    info.compress_type = zipfile.ZIP_DEFLATED
    return info


def exit_on(sub, problems):
    if not problems:
        return
    for problem in problems:
        print(f"{sub}: {problem}", file=sys.stderr)
    sys.exit(1)


def cmd_package(args):
    exe = Path(args.exe)
    if not exe.is_file():
        raise GateBroken(f"exe {exe} not found")
    doc = manifest()
    packs = check(doc)
    if packs:
        exit_on("package", [f"third-party packs do not match the manifest: {'; '.join(packs)}; "
                            "run tools/fetch_assets.py"])
    data = exe.read_bytes()
    exit_on("package", exe_problems(data, exe.name.endswith(".exe")))
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    path = out / f"{args.name}.zip"
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr(entry(f"{args.name}/{exe.name}", exe, 0o755), data)
        for rel in expected_assets(doc):
            source = REPO / "assets" / rel
            if not source.is_file():
                raise GateBroken(f"asset {source} not found")
            archive.writestr(entry(f"{args.name}/assets/{rel}", source, 0o644), source.read_bytes())
    print(f"package: wrote {path}")
    verify(path, doc, "package")


def verify(path, doc, sub):
    if not path.is_file():
        raise GateBroken(f"zip {path} not found")
    try:
        archive = zipfile.ZipFile(path)
    except zipfile.BadZipFile as error:
        raise GateBroken(f"{path} is not a zip: {error}") from error
    with archive:
        infos = {info.filename: info for info in archive.infolist()}
        tops = sorted({name.split("/", 1)[0] for name in infos})
        if len(tops) != 1:
            exit_on(sub, [f"top folder: expected one, found {tops}"])
        top = tops[0]
        exes = [f"{top}/{name}" for name in EXE_NAMES if f"{top}/{name}" in infos]
        assets = expected_assets(doc)
        expected = {f"{top}/assets/{rel}" for rel in assets} | set(exes)
        problems = []
        if len(exes) != 1:
            problems.append(f"exe entry: expected exactly one of {top}/gta_like[.exe], found {exes}")
        missing = sorted(expected - set(infos))
        unexpected = sorted(set(infos) - expected)
        if missing:
            problems.append(f"missing: {', '.join(missing)}")
        if unexpected:
            problems.append(f"unexpected: {', '.join(unexpected)}")
        shas = pack_shas(doc)
        for rel in assets:
            name = f"{top}/assets/{rel}"
            if name not in infos:
                continue
            actual, wanted = sha256_bytes(archive.read(name)), expected_sha(rel, shas)
            if actual != wanted:
                problems.append(f"sha256 {rel}: zip {actual}, expected {wanted}")
        for pack in doc["packs"]:
            if f"{top}/assets/third_party/{pack['name']}/{pack['license_file']}" not in infos:
                problems.append(f"licence missing: {pack['name']}")
        for name in exes:
            problems += exe_problems(archive.read(name), name.endswith(".exe"))
            mode = infos[name].external_attr >> 16
            if not name.endswith(".exe") and mode & 0o111 == 0:
                problems.append(f"exec bit: mode {oct(mode)}")
    exit_on(sub, problems)
    print(f"{sub}: OK {path.name}: {len(infos)} entries (exe + {len(assets)} assets), "
          f"{path.stat().st_size / 1e6:.1f} MB")
    print(f"{sub}: correctness: allowlist exact, sha256 match (manifest for packs, checkout for tracked), "
          f"licences for {len(doc['packs'])} packs, no dynamic imports, GUI subsystem on Windows")


def cmd_verify(args):
    verify(Path(args.zip), manifest(), "verify")


def cmd_smoke(args):
    folder = Path(args.dir)
    exe = next((folder / name for name in EXE_NAMES if (folder / name).is_file()), None)
    if exe is None:
        raise GateBroken(f"no gta_like[.exe] in {folder}")
    try:
        allows = [re.compile(pattern) for pattern in args.allow]
    except re.error as error:
        raise GateBroken(f"invalid --allow regex: {error}") from error
    log = Path(args.log).resolve()
    console = log.with_suffix(".console.log")
    game_log = folder / GAME_LOG
    try:
        game_log.unlink(missing_ok=True)
    except OSError as error:
        raise GateBroken(f"cannot remove a stale {game_log}: {error}") from error
    env = {key: value for key, value in os.environ.items() if key not in ("CARGO_MANIFEST_DIR", "BEVY_ASSET_ROOT")}
    cwd = tempfile.mkdtemp(prefix="gta_like_smoke_")
    argv = [str(exe.resolve()), "--settings-id", "gta_like_smoke", *args.game_args]
    started = time.monotonic()
    try:
        with open(console, "wb") as out:
            game = subprocess.Popen(argv, cwd=cwd, env=env, stdout=out, stderr=subprocess.STDOUT)
            while game.poll() is None and time.monotonic() - started < args.seconds:
                time.sleep(POLL_SECONDS)
            code = game.poll()
            if code is None:
                game.kill()
                game.wait()
    except OSError as error:
        raise GateBroken(f"cannot start {exe} or write {console}: {error}") from error
    finally:
        shutil.rmtree(cwd, ignore_errors=True)
    written = game_log.is_file()
    if written:
        shutil.copyfile(game_log, log)
    lines = [ANSI.sub("", line) for line in log.read_text(encoding="utf-8", errors="replace").splitlines()] \
        if written else []
    if code is not None:
        tail = "\n".join(lines[-TAIL_LINES:])
        console_tail = "\n".join(console.read_text(encoding="utf-8", errors="replace").splitlines()[-TAIL_LINES:])
        exit_on("smoke", [f"liveness: exited after {time.monotonic() - started:.1f} s with code {code}; "
                          f"last lines of {game_log}:\n{tail}\nlast lines of {console}:\n{console_tail}"])
    if not written:
        exit_on("smoke", [f"log file: the game wrote no {game_log} (console output in {console})"])
    problems = [f"error line: {line}" for line in lines
                if BAD_LINE.search(line) and not any(allow.search(line) for allow in allows)]
    found = {text: next((line for line in lines if text in line), None) for text in args.expect}
    problems += [f"expect: {text!r} on no line of {log}" for text, line in found.items() if line is None]
    exit_on("smoke", problems)
    for line in found.values():
        print(f"smoke: expect matched: {line.strip()}")
    adapter = next((line.strip() for line in lines if "AdapterInfo {" in line), "none")
    warns = sum(1 for line in lines if re.search(r"\bWARN\b", line))
    print(f"smoke: adapter: {adapter}")
    print(f"smoke: OK {exe.name} alive {args.seconds} s; liveness: alive at deadline; log: {GAME_LOG} next to the "
          f"exe, copied to {log}; state: {len(found)} expect(s) matched; correctness: 0 ERROR/panic lines "
          f"({len(allows)} allow pattern(s)), {warns} WARN lines")


def main():
    argv = sys.argv[1:]
    game_args = []
    if "--" in argv:
        split = argv.index("--")
        argv, game_args = argv[:split], argv[split + 1:]
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    subs = parser.add_subparsers(dest="sub", required=True)
    package = subs.add_parser("package", help="zip exe + assets/ and verify the zip")
    package.add_argument("--exe", required=True)
    package.add_argument("--name", required=True, help="zip basename and its top folder")
    package.add_argument("--out", required=True, help="output directory")
    verify_cmd = subs.add_parser("verify", help="check a release zip")
    verify_cmd.add_argument("zip")
    smoke = subs.add_parser("smoke", help="boot the unpacked game and scan its log (game args after --)")
    smoke.add_argument("dir", help="unpacked top folder holding gta_like[.exe] and assets/")
    smoke.add_argument("--seconds", type=float, required=True)
    smoke.add_argument("--log", required=True)
    smoke.add_argument("--expect", action="append", default=[], help="text that must appear on some log line")
    smoke.add_argument("--allow", action="append", default=[], help="regex exempting an ERROR/panic line")
    args = parser.parse_args(argv)
    if game_args and args.sub != "smoke":
        parser.error("game args after -- are only for smoke")
    args.game_args = game_args
    try:
        {"package": cmd_package, "verify": cmd_verify, "smoke": cmd_smoke}[args.sub](args)
    except GateBroken as error:
        print(f"{args.sub}: GATE BROKEN: {error}", file=sys.stderr)
        sys.exit(2)


if __name__ == "__main__":
    main()
