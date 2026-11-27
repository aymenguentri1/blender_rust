#!/usr/bin/env python3
"""Rewrite the progress block at the top of README.md from the GitHub issues.

Headline = mean per-epic completion (epics with no tasks planned count 0 %), plus one row per epic from the
"Part of epic #N" line of each task. Usage: python3 scripts/progress.py [--print]
Needs `gh` and GH_TOKEN. The planned backlog grows as new areas are planned, so the % is of what is planned.
"""
import json, re, subprocess, sys

REPO = "aymenguentri1/blender_rust"
START, END = "<!-- progress:start -->", "<!-- progress:end -->"


def issues():
    out = subprocess.run(
        ["gh", "api", "--paginate", f"repos/{REPO}/issues?state=all&per_page=100"],
        check=True, capture_output=True, text=True).stdout
    # --paginate prints one JSON array per page back to back
    dec, pos, res = json.JSONDecoder(), 0, []
    while pos < len(out):
        arr, pos = dec.raw_decode(out, pos)
        res += arr
        while pos < len(out) and out[pos].isspace():
            pos += 1
    return [i for i in res if "pull_request" not in i]


def bar(done, total, n=12):
    k = round(n * done / total) if total else 0
    return "▰" * k + "▱" * (n - k)


def pct(done, total):
    return 100.0 * done / total if total else 0.0


def build(items):
    labels = lambda i: {l["name"] for l in i["labels"]}
    epics = {i["number"]: i for i in items if "epic" in labels(i)}
    tasks = [i for i in items if "epic" not in labels(i)]
    done = sum(i["state"] == "closed" for i in tasks)
    total = len(tasks)
    per = {n: [0, 0] for n in epics}
    for t in tasks:
        m = re.search(r"epic #(\d+)", t.get("body") or "")
        if m and int(m.group(1)) in per:
            per[int(m.group(1))][1] += 1
            per[int(m.group(1))][0] += t["state"] == "closed"
    # headline: mean of the per-epic % over ALL epics (an epic with nothing planned yet counts as 0 %)
    p = sum(pct(d, t) for d, t in per.values()) / len(per) if per else 0.0
    color = "red" if p < 25 else "orange" if p < 50 else "yellow" if p < 75 else "brightgreen"
    lines = [
        START,
        '<h2 align="center">Blender in Rust: progress</h2>',
        '<p align="center">',
        f'<img alt="progress" src="https://img.shields.io/badge/ported-{p:.1f}%25-{color}?style=for-the-badge&logo=blender&logoColor=white">',
        f'<img alt="tasks" src="https://img.shields.io/badge/tasks-{done}%2F{total}-blue?style=for-the-badge">',
        f'<img alt="epics" src="https://img.shields.io/badge/epics-{sum(1 for n in epics if epics[n]["state"] == "closed")}%2F{len(epics)}-8a2be2?style=for-the-badge">',
        "</p>",
        "",
        f"**`{bar(p, 100, 30)}` {p:.1f} %** is the mean completion of the {len(epics)} epics ({done} of {total} planned tasks merged).",
        "",
        "| Area | Progress | Done |",
        "| --- | --- | ---: |",
    ]
    for n, (d, t) in sorted(per.items(), key=lambda kv: (-pct(*kv[1]), kv[0])):
        if t:
            lines.append(f"| {epics[n]['title']} | `{bar(d, t)}` {pct(d, t):.0f} % | {d}/{t} |")
    lines += ["", "<sub>Every task is ported from a real file in the Blender source tree (about 21 000 files exist, "
              "so this is the progress of the planned scope, not of all of Blender). "
              "Regenerate with `python3 scripts/progress.py`; the % can drop when new tasks are planned.</sub>", END]
    return "\n".join(lines)


def main():
    block = build(issues())
    if "--print" in sys.argv:
        print(block)
        return
    text = open("README.md", encoding="utf-8").read()
    if START in text:
        text = re.sub(re.escape(START) + r".*?" + re.escape(END), lambda _: block, text, flags=re.S)
    else:
        first, _, rest = text.partition("\n")
        text = f"{first}\n\n{block}\n{rest}"
    open("README.md", "w", encoding="utf-8").write(text)


if __name__ == "__main__":
    main()
