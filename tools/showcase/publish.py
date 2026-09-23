"""Publish recorded showcase GIFs: force-push them as the single commit of the orphan branch `media`
and rewrite the README showcase blocks (a hero clip under the title, the rest next to their slice
lines in "## Статус").

Usage:
  python tools/showcase/publish.py --dry-run      build the commit on local branch media-dry-run,
                                                  print the files and the README diff, push nothing
  python tools/showcase/publish.py                push `media`, rewrite README.md (commit it yourself)
  python tools/showcase/publish.py --hero city    choose the hero clip (default: shooting)

Each scenario module names its README slice line (ANCHOR) and caption (CAPTION). Only scenarios with
a GIF in the output dir are published; the others keep their README block as it is.
"""

import argparse
import difflib
import html
from pathlib import Path
import re
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import record  # noqa: E402

REPO = record.REPO
README = REPO / "README.md"
BRANCH = "media"
DRY_RUN_BRANCH = "media-dry-run"
STATUS_HEADING = "## Статус"
HERO_WIDTH = 720
STATUS_WIDTH = 480
HERO_START, HERO_END = "<!-- SHOWCASE-HERO:START -->", "<!-- SHOWCASE-HERO:END -->"


def git(*args, stdin=None):
    return subprocess.run(
        ["git", *args], cwd=REPO, input=stdin, capture_output=True, check=True,
    ).stdout.decode("utf-8").strip()


def raw_base():
    """https://raw.githubusercontent.com/<owner>/<repo>/media from the origin URL (ssh or https)."""
    url = git("remote", "get-url", "origin")
    match = re.search(r"github\.com[:/]([^/]+)/(.+?)(?:\.git)?$", url)
    if not match:
        raise SystemExit(f"origin {url} is not a GitHub remote")
    return f"https://raw.githubusercontent.com/{match.group(1)}/{match.group(2)}/{BRANCH}"


def media_commit(gifs):
    """Orphan commit holding exactly `gifs` (name -> path); nothing touches the index or work tree."""
    blobs = {name: git("hash-object", "-w", "--no-filters", str(path)) for name, path in gifs.items()}
    tree_input = "".join(f"100644 blob {sha}\t{name}.gif\n" for name, sha in sorted(blobs.items()))
    tree = git("mktree", stdin=tree_input.encode("utf-8"))
    commit = git("commit-tree", tree, "-m", "showcase media (force-pushed, single commit)")
    return commit, blobs


def status_block(name, url, caption):
    caption = html.escape(caption)
    return [
        f"  <!-- SHOWCASE:{name}:START -->",
        f'  <img src="{url}" width="{STATUS_WIDTH}" alt="{caption}"><br><sub>{caption}</sub>',
        f"  <!-- SHOWCASE:{name}:END -->",
    ]


def hero_block(url, caption):
    caption = html.escape(caption)
    return [HERO_START, "", f'<p align="center"><img src="{url}" width="{HERO_WIDTH}" alt="{caption}"></p>', "", HERO_END]


def find_block(lines, start, end):
    """(first, last) line indices of a marker pair (stripped match), or None."""
    starts = [i for i, line in enumerate(lines) if line.strip() == start]
    if not starts:
        return None
    last = next((i for i in range(starts[0], len(lines)) if lines[i].strip() == end), None)
    if last is None:
        raise SystemExit(f"README: {start} has no {end}")
    return starts[0], last


def set_hero(lines, url, caption):
    block = hero_block(url, caption)
    found = find_block(lines, HERO_START, HERO_END)
    if found:
        return lines[:found[0]] + block + lines[found[1] + 1:]
    title = next((i for i, line in enumerate(lines) if line.startswith("# ")), None)
    if title is None:
        raise SystemExit("README has no '# ' title line for the hero clip")
    return lines[:title + 1] + [""] + block + lines[title + 1:]


def drop_status(lines, name):
    found = find_block(lines, f"<!-- SHOWCASE:{name}:START -->", f"<!-- SHOWCASE:{name}:END -->")
    return lines if not found else lines[:found[0]] + lines[found[1] + 1:]


def set_status(lines, name, anchor, url, caption):
    """Replace the clip's block, or insert it as a continuation of its slice line in "## Статус"."""
    block = status_block(name, url, caption)
    found = find_block(lines, f"<!-- SHOWCASE:{name}:START -->", f"<!-- SHOWCASE:{name}:END -->")
    if found:
        return lines[:found[0]] + block + lines[found[1] + 1:]
    heading = next((i for i, line in enumerate(lines) if line.strip() == STATUS_HEADING), None)
    if heading is None:
        raise SystemExit(f"README has no {STATUS_HEADING!r} section")
    section_end = next((i for i in range(heading + 1, len(lines)) if lines[i].startswith("## ")), len(lines))
    item = next((i for i in range(heading + 1, section_end) if lines[i].startswith(anchor)), None)
    if item is None:
        raise SystemExit(f"scenario {name}: no line starting with {anchor!r} in {STATUS_HEADING}")
    end = item + 1
    while end < section_end and lines[end].startswith("  ") and lines[end].strip():
        end += 1
    return lines[:end] + block + lines[end:]


def rewrite_readme(text, clips, hero):
    lines = text.splitlines()
    for name, clip in clips.items():
        if name == hero:
            lines = set_hero(drop_status(lines, name), clip["url"], clip["caption"])
        else:
            lines = set_status(lines, name, clip["anchor"], clip["url"], clip["caption"])
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--dry-run", action="store_true", help="local branch media-dry-run and a README diff only")
    parser.add_argument("--hero", default="shooting", help="scenario shown under the README title")
    parser.add_argument("--gifs", type=Path, help="GIF dir (default <cargo target dir>/showcase)")
    args = parser.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    folder = (args.gifs or record.target_dir() / "showcase").resolve()
    scenarios = record.load_scenarios(record.scenario_names())
    gifs = {s.NAME: folder / f"{s.NAME}.gif" for s in scenarios if (folder / f"{s.NAME}.gif").is_file()}
    if not gifs:
        raise SystemExit(f"no scenario GIFs in {folder}; run tools/showcase/record.py --all first")
    if args.hero not in gifs:
        raise SystemExit(f"hero clip {args.hero!r} has no GIF in {folder}")
    oversized = [f"{p} ({p.stat().st_size} B)" for p in gifs.values() if p.stat().st_size > record.MAX_BYTES]
    if oversized:
        raise SystemExit(f"over {record.MAX_BYTES} B: {oversized}")

    commit, blobs = media_commit(gifs)
    base = raw_base()
    clips = {
        s.NAME: {"url": f"{base}/{s.NAME}.gif?v={blobs[s.NAME][:8]}", "caption": s.CAPTION, "anchor": s.ANCHOR}
        for s in scenarios if s.NAME in gifs
    }
    old = README.read_text(encoding="utf-8")
    new = rewrite_readme(old, clips, args.hero)

    print(f"media commit {commit}:")
    print(git("ls-tree", "-l", commit))
    for name, clip in clips.items():
        print(f"{name}: {clip['url']}")
    if args.dry_run:
        git("branch", "-f", DRY_RUN_BRANCH, commit)
        print(f"\n[dry-run] local branch {DRY_RUN_BRANCH} -> {commit}; nothing pushed, README.md untouched")
        sys.stdout.writelines(difflib.unified_diff(
            old.splitlines(keepends=True), new.splitlines(keepends=True), "README.md", "README.md (would be)",
        ))
        return
    subprocess.run(["git", "push", "--force", "origin", f"{commit}:refs/heads/{BRANCH}"], cwd=REPO, check=True)
    newline = "\r\n" if b"\r\n" in README.read_bytes() else "\n"
    README.write_text(new, encoding="utf-8", newline=newline)
    print(f"pushed {BRANCH}; README.md rewritten, commit it on your branch")


if __name__ == "__main__":
    main()
