"""Fetch, verify and unpack the third-party asset packs listed in assets/third_party/manifest.ron.

Usage:
  python tools/fetch_assets.py [--cache DIR ...]   fetch missing or broken packs (idempotent)
  python tools/fetch_assets.py --check             verify installed packs offline
  python tools/fetch_assets.py --validate-only F   parse and schema-check a manifest file

The schema rules mirror gta_sim::config::manifest::ThirdPartyManifest::validate.
A pack may carry a `rig` record; --check and installation compare it with the GLB bytes.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import sys
import time
import urllib.request
import zipfile

REPO = Path(__file__).resolve().parents[1]
THIRD_PARTY = REPO / "assets" / "third_party"
MANIFEST = THIRD_PARTY / "manifest.ron"
DEFAULT_CACHE = REPO / "target" / "asset-cache"
USER_AGENT = "gta-like-fetch-assets"
RETRY_PAUSES = (2, 4, 8)

PACK_KEYS = {"name", "version", "page", "url", "archive_sha256", "license", "license_file", "files"}
FILE_KEYS = {"archive", "path", "sha256"}
OPTIONAL_PACK_KEYS = frozenset({"rig"})
RIG_KEYS = {"models", "skinned_meshes", "joints", "clips"}
GLB_JSON_CHUNK = 0x4E4F534A
IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
SHA256 = re.compile(r"[0-9a-f]{64}")
NAME = re.compile(r"[a-z0-9-]+")


class ManifestError(Exception):
    pass


# ---------------------------------------------------------------- RON subset parser

class Parser:
    def __init__(self, text):
        self.text = text
        self.pos = 0

    def fail(self, message):
        line = self.text.count("\n", 0, self.pos) + 1
        raise ManifestError(f"line {line}: {message}")

    def skip(self):
        while self.pos < len(self.text):
            if self.text[self.pos].isspace():
                self.pos += 1
            elif self.text.startswith("//", self.pos):
                end = self.text.find("\n", self.pos)
                self.pos = len(self.text) if end < 0 else end
            else:
                return

    def peek(self):
        self.skip()
        return self.text[self.pos] if self.pos < len(self.text) else ""

    def expect(self, char):
        if self.peek() != char:
            self.fail(f"expected {char!r}, found {self.peek()!r}")
        self.pos += 1

    def value(self):
        char = self.peek()
        if char == "(":
            return self.struct()
        if char == "[":
            return self.list()
        if char == '"':
            return self.string()
        match = IDENT.match(self.text, self.pos)
        if match:
            self.pos = match.end()
            ident = match.group()
            if ident == "Some" and self.peek() == "(":
                self.expect("(")
                inner = self.value()
                self.expect(")")
                return inner
            if self.peek() == "(":
                self.fail(f"named struct {ident!r} is not supported")
            if ident == "None":
                return None
            return ident
        self.fail(f"unexpected {char!r}")

    def struct(self):
        self.expect("(")
        out = {}
        while self.peek() != ")":
            match = IDENT.match(self.text, self.pos)
            if not match:
                self.fail("expected a field name")
            key = match.group()
            self.pos = match.end()
            if key in out:
                self.fail(f"duplicate field {key!r}")
            self.expect(":")
            out[key] = self.value()
            if self.peek() == ",":
                self.pos += 1
            elif self.peek() != ")":
                self.fail("expected ',' or ')'")
        self.pos += 1
        return out

    def list(self):
        self.expect("[")
        out = []
        while self.peek() != "]":
            out.append(self.value())
            if self.peek() == ",":
                self.pos += 1
            elif self.peek() != "]":
                self.fail("expected ',' or ']'")
        self.pos += 1
        return out

    def string(self):
        self.expect('"')
        out = []
        while True:
            if self.pos >= len(self.text):
                self.fail("unterminated string")
            char = self.text[self.pos]
            self.pos += 1
            if char == '"':
                return "".join(out)
            if char == "\\":
                escaped = self.text[self.pos:self.pos + 1]
                if escaped not in ('"', "\\"):
                    self.fail(f"unsupported escape \\{escaped}")
                out.append(escaped)
                self.pos += 1
            else:
                out.append(char)


def parse_ron(text):
    parser = Parser(text)
    doc = parser.value()
    if parser.peek():
        parser.fail("trailing content")
    return doc


# ---------------------------------------------------------------- schema (mirror of validate())

def safe_path(value):
    return (isinstance(value, str) and value and "\\" not in value and ":" not in value
            and not value.startswith("/")
            and all(seg not in ("", ".", "..") for seg in value.split("/")))


def exact_keys(obj, keys, where, optional=frozenset()):
    if not isinstance(obj, dict):
        raise ManifestError(f"{where}: expected a struct")
    extra, missing = set(obj) - keys - optional, keys - set(obj)
    if extra or missing:
        raise ManifestError(f"{where}: unknown fields {sorted(extra)}, missing fields {sorted(missing)}")


def check_schema(doc):
    exact_keys(doc, {"packs"}, "manifest")
    packs = doc["packs"]
    if not isinstance(packs, list) or not packs:
        raise ManifestError("packs is empty")
    names = set()
    for pack in packs:
        exact_keys(pack, PACK_KEYS, "pack", OPTIONAL_PACK_KEYS)
        name = pack["name"]
        if name in names:
            raise ManifestError(f"duplicate pack name {name!r}")
        names.add(name)
        if not isinstance(name, str) or not NAME.fullmatch(name):
            raise ManifestError(f"pack {name!r}: name must match ^[a-z0-9-]+$")
        if not pack["version"]:
            raise ManifestError(f"pack {name}: version is empty")
        if pack["page"] != f"https://kenney.nl/assets/{name}":
            raise ManifestError(f"pack {name}: page {pack['page']!r}")
        prefix = f"https://kenney.nl/media/pages/assets/{name}/"
        if not pack["url"].startswith(prefix) or not pack["url"].endswith(".zip"):
            raise ManifestError(f"pack {name}: url {pack['url']!r} must start with {prefix!r} and end with .zip")
        if pack["license"] != "CC0":
            raise ManifestError(f"pack {name}: license {pack['license']!r} is not CC0")
        if not SHA256.fullmatch(pack["archive_sha256"]):
            raise ManifestError(f"pack {name}: archive_sha256 {pack['archive_sha256']!r} is not 64 lowercase hex digits")
        files = pack["files"]
        if not isinstance(files, list) or not files:
            raise ManifestError(f"pack {name}: files is empty")
        paths, archives = set(), set()
        for entry in files:
            exact_keys(entry, FILE_KEYS, f"pack {name} file")
            for field in ("path", "archive"):
                if not safe_path(entry[field]):
                    raise ManifestError(f"pack {name}: unsafe {field} {entry[field]!r}")
            if not SHA256.fullmatch(entry["sha256"]):
                raise ManifestError(f"pack {name}: sha256 {entry['sha256']!r} of {entry['path']} is not 64 lowercase hex digits")
            if entry["path"] in paths:
                raise ManifestError(f"pack {name}: duplicate path {entry['path']!r}")
            if entry["archive"] in archives:
                raise ManifestError(f"pack {name}: duplicate archive {entry['archive']!r}")
            paths.add(entry["path"])
            archives.add(entry["archive"])
        if pack["license_file"] not in paths:
            raise ManifestError(f"pack {name}: license_file {pack['license_file']!r} is not listed in files")
        rig = pack.get("rig")
        if rig is None:
            continue
        exact_keys(rig, RIG_KEYS, f"pack {name} rig")
        check_rig(name, rig, paths)


def check_names(pack, field, items):
    if not isinstance(items, list) or not items:
        raise ManifestError(f"pack {pack}: rig {field} is empty")
    seen = set()
    for item in items:
        if not isinstance(item, str) or not item:
            raise ManifestError(f"pack {pack}: rig {field} has an empty name")
        if item in seen:
            raise ManifestError(f"pack {pack}: rig {field} has duplicate {item!r}")
        seen.add(item)


def check_rig(pack, rig, paths):
    for field in ("models", "skinned_meshes", "joints", "clips"):
        check_names(pack, field, rig[field])
    for model in rig["models"]:
        if model not in paths:
            raise ManifestError(f"pack {pack}: rig model {model!r} is not listed in files")
        if not model.endswith(".glb"):
            raise ManifestError(f"pack {pack}: rig model {model!r} is not a .glb")


def load_manifest(path):
    doc = parse_ron(Path(path).read_text(encoding="utf-8"))
    check_schema(doc)
    return doc


# ---------------------------------------------------------------- installed packs

def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def listing(directory):
    return {p.relative_to(directory).as_posix() for p in directory.rglob("*") if p.is_file()}


def pack_problems(pack):
    directory = THIRD_PARTY / pack["name"]
    if not directory.is_dir():
        return [f"{pack['name']}: not installed"]
    expected = {f["path"]: f["sha256"] for f in pack["files"]}
    present = listing(directory)
    problems = [f"{pack['name']}: missing {p}" for p in sorted(set(expected) - present)]
    problems += [f"{pack['name']}: unexpected {p}" for p in sorted(present - set(expected))]
    for rel in sorted(set(expected) & present):
        actual = sha256_file(directory / rel)
        if actual != expected[rel]:
            problems.append(f"{pack['name']}: {rel} sha256 {actual}, manifest {expected[rel]}")
    if not problems and pack.get("rig"):
        problems += rig_problems(pack, lambda p: (directory / p).read_bytes())
    return problems


def glb_json(model, data):
    if len(data) < 20:
        raise ManifestError(f"{model}: not a GLB file")
    magic, version, length = struct.unpack_from("<4sII", data, 0)
    if magic != b"glTF" or version != 2 or length != len(data):
        raise ManifestError(f"{model}: not a glTF 2 binary (magic {magic!r}, version {version}, length {length})")
    chunk_length, chunk_type = struct.unpack_from("<II", data, 12)
    if chunk_type != GLB_JSON_CHUNK:
        raise ManifestError(f"{model}: first GLB chunk is not JSON")
    return json.loads(data[20:20 + chunk_length])


def rig_facts(gltf):
    nodes = gltf.get("nodes", [])
    skinned = [node.get("name") for node in nodes if "skin" in node]
    joints_per_skin = [[nodes[j].get("name") for j in skin["joints"]] for skin in gltf.get("skins", [])]
    clips = [animation.get("name") for animation in gltf.get("animations", [])]
    return skinned, joints_per_skin, clips


def rig_problems(pack, read):
    rig = pack["rig"]
    problems = []
    for model in rig["models"]:
        skinned, joints_per_skin, clips = rig_facts(glb_json(model, read(model)))
        where = f"{pack['name']}: {model}: rig"
        if skinned != rig["skinned_meshes"]:
            problems.append(f"{where} skinned_meshes {skinned}, manifest {rig['skinned_meshes']}")
        if len(joints_per_skin) != len(skinned):
            problems.append(f"{where} skins {len(joints_per_skin)}, skinned meshes {len(skinned)}")
        for joints in joints_per_skin:
            if joints != rig["joints"]:
                problems.append(f"{where} joints {joints}, manifest {rig['joints']}")
        if clips != rig["clips"]:
            problems.append(f"{where} clips {clips}, manifest {rig['clips']}")
    return problems


def check(doc):
    names = {pack["name"] for pack in doc["packs"]}
    problems = []
    for entry in sorted(THIRD_PARTY.iterdir()) if THIRD_PARTY.is_dir() else []:
        if entry.name != "manifest.ron" and not (entry.is_dir() and entry.name in names):
            problems.append(f"unexpected entry {entry.relative_to(REPO).as_posix()}")
    for pack in doc["packs"]:
        problems += pack_problems(pack)
    return problems


# ---------------------------------------------------------------- fetch

def cached_archive(pack, caches):
    basename = pack["url"].rsplit("/", 1)[1]
    for cache in caches:
        candidate = Path(cache) / basename
        if not candidate.is_file():
            continue
        actual = sha256_file(candidate)
        if actual == pack["archive_sha256"]:
            return candidate
        print(f"{pack['name']}: cached {candidate} archive sha256 {actual}, manifest {pack['archive_sha256']}; ignored",
              file=sys.stderr)
    return None


def download(pack):
    basename = pack["url"].rsplit("/", 1)[1]
    DEFAULT_CACHE.mkdir(parents=True, exist_ok=True)
    part = DEFAULT_CACHE / (basename + ".part")
    last_error = None
    for attempt, pause in enumerate((0, *RETRY_PAUSES)):
        time.sleep(pause)
        try:
            req = urllib.request.Request(pack["url"], headers={"User-Agent": USER_AGENT})
            with urllib.request.urlopen(req, timeout=120) as response, open(part, "wb") as out:
                shutil.copyfileobj(response, out)
            break
        except OSError as error:
            last_error = error
            print(f"{pack['name']}: download attempt {attempt + 1} failed: {error}", file=sys.stderr)
    else:
        raise ManifestError(f"{pack['name']}: download failed: {last_error}")
    actual = sha256_file(part)
    if actual != pack["archive_sha256"]:
        part.unlink()
        raise ManifestError(f"{pack['name']}: archive sha256 {actual}, manifest {pack['archive_sha256']}")
    final = DEFAULT_CACHE / basename
    os.replace(part, final)
    return final


def extract(pack, archive):
    name = pack["name"]
    tmp = THIRD_PARTY / f".tmp-{name}"
    old = THIRD_PARTY / f".old-{name}"
    final = THIRD_PARTY / name
    shutil.rmtree(tmp, ignore_errors=True)
    try:
        with zipfile.ZipFile(archive) as zf:
            members = set(zf.namelist())
            for entry in pack["files"]:
                if entry["archive"] not in members:
                    raise ManifestError(f"{name}: {entry['archive']} is not in {archive.name}")
                data = zf.read(entry["archive"])
                actual = sha256_bytes(data)
                if actual != entry["sha256"]:
                    raise ManifestError(f"{name}: {entry['path']} sha256 {actual}, manifest {entry['sha256']}")
                target = tmp / entry["path"]
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
        if pack.get("rig"):
            problems = rig_problems(pack, lambda p: (tmp / p).read_bytes())
            if problems:
                raise ManifestError("\n".join(problems))
    except BaseException:
        shutil.rmtree(tmp, ignore_errors=True)
        raise
    shutil.rmtree(old, ignore_errors=True)
    moved_old = False
    try:
        if final.exists():
            os.replace(final, old)
            moved_old = True
        os.replace(tmp, final)
    except BaseException:
        if moved_old:
            os.replace(old, final)
        shutil.rmtree(tmp, ignore_errors=True)
        raise
    shutil.rmtree(old, ignore_errors=True)


def fetch(doc, caches):
    for pack in doc["packs"]:
        if not pack_problems(pack):
            print(f"{pack['name']}: up to date, skipped")
            continue
        archive = cached_archive(pack, [*caches, DEFAULT_CACHE]) or download(pack)
        extract(pack, archive)
        print(f"{pack['name']}: installed from {archive}")
    problems = check(doc)
    if problems:
        raise ManifestError("after fetch:\n" + "\n".join(problems))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="verify installed packs, no network")
    mode.add_argument("--validate-only", type=Path, metavar="FILE", help="parse and schema-check FILE only")
    parser.add_argument("--cache", action="append", default=[], metavar="DIR", help="directory with pack zips")
    args = parser.parse_args()
    try:
        if args.validate_only:
            load_manifest(args.validate_only)
            print(f"{args.validate_only}: valid")
            return 0
        doc = load_manifest(MANIFEST)
        if args.check:
            problems = check(doc)
            for problem in problems:
                print(problem, file=sys.stderr)
            if problems:
                print("run `python tools/fetch_assets.py` to (re)install the packs", file=sys.stderr)
                return 1
            print("third-party packs match the manifest")
            return 0
        fetch(doc, args.cache)
        return 0
    except (ManifestError, OSError, zipfile.BadZipFile) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
