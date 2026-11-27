#!/usr/bin/env python3
"""Rewrite the parity block of README.md: Blender source files (source/blender) cited by the Rust port vs all of them.

Usage: python3 scripts/parity.py [--print]. Needs the reference clone (scripts/blender_ref.sh setup).
A file counts as covered when a src/*.rs file names it (path or unique basename). It is a lower bound on
ported scope: file count, not lines, and headers count as files."""
import collections, glob, os, re, subprocess, sys

START, END = "<!-- parity:start -->", "<!-- parity:end -->"
EXT = r"\.(c|cc|cpp|h|hh|hpp|py)$"


def blender_files():
    ref = os.environ.get("BLENDER_REF_DIR") or os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "blender")
    out = subprocess.run(["git", "-C", ref, "ls-tree", "-r", "--name-only", "HEAD"], check=True, capture_output=True, text=True).stdout
    return [n for n in out.split("\n") if n.startswith("source/blender/") and re.search(EXT, n)]


def cited(files):
    known, base = set(files), collections.defaultdict(list)
    for n in files:
        base[os.path.basename(n)].append(n)
    res = set()
    for f in glob.glob("src/**/*.rs", recursive=True):
        text = open(f, errors="ignore").read()
        for m in re.findall(r"source/blender/[\w/.\-]+|\b[\w\-]+\.(?:cc|cpp|c|hh|h|hpp|py)\b", text):
            if m in known:
                res.add(m)
            elif "/" not in m and len(base.get(m, [])) == 1:
                res.add(base[m][0])
    return res


def build():
    files = blender_files()
    got = cited(files)
    mod = lambda n: n.split("/")[2]
    tot, hit = collections.Counter(map(mod, files)), collections.Counter(map(mod, got))
    rows = sorted(tot, key=lambda k: -tot[k])
    loc = sum(len(open(f, errors="ignore").read().splitlines()) for f in glob.glob("src/**/*.rs", recursive=True))
    out = [f"**{len(got)} of {len(files)} Blender source files ({100 * len(got) / len(files):.1f} %)** are referenced by the Rust port "
           f"(`{loc}` lines of Rust). Left: **{len(files) - len(got)}** files, i.e. about **{100 - 100 * len(got) / len(files):.0f} %** of `source/blender`.", "",
           "| Blender module | Files referenced | Total | Covered |", "| --- | ---: | ---: | ---: |"]
    out += [f"| `{k}` | {hit[k]} | {tot[k]} | {100 * hit[k] / tot[k]:.1f} % |" for k in rows]
    out += ["", "<sub>File count, not lines: a lower bound of what is ported, and headers count as files. Regenerate with `python3 scripts/parity.py`.</sub>"]
    return "\n".join(out)


if __name__ == "__main__":
    block = build()
    if "--print" in sys.argv:
        print(block)
    else:
        t = open("README.md").read()
        s, e = t.index(START) + len(START), t.index(END)
        open("README.md", "w").write(t[:s] + "\n" + block + "\n" + t[e:])
