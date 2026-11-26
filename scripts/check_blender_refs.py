#!/usr/bin/env python3
"""CI gate: a PR that changes code must cite real files from the Blender reference repo.

Reads the PR body, takes the section "## Blender source ported from", extracts file/dir paths
and verifies each exists in https://github.com/aymenguentri1/blender (branch main).
PRs that only touch docs/CI/meta files are exempt.
"""
import json, os, re, sys, urllib.request

REF = os.environ.get("BLENDER_REF_REPO", "aymenguentri1/blender")
BRANCH = os.environ.get("BLENDER_REF_BRANCH", "main")
TOKEN = os.environ.get("GITHUB_TOKEN", "")
EXEMPT = re.compile(r"^(\.github/|docs/|scripts/|LICENSE|AGENTS\.md|CLAUDE\.md|README\.md|\.gitattributes|\.gitignore)")

def api(url):
    h = {"Accept": "application/vnd.github+json"}
    if TOKEN:
        h["Authorization"] = "Bearer " + TOKEN
    return json.load(urllib.request.urlopen(urllib.request.Request(url, headers=h)))

def main():
    ev = json.load(open(os.environ["GITHUB_EVENT_PATH"]))
    pr = ev.get("pull_request")
    if not pr:
        print("not a PR, skipping"); return 0
    repo = os.environ["GITHUB_REPOSITORY"]
    files, page = [], 1
    while True:
        chunk = api(f"https://api.github.com/repos/{repo}/pulls/{pr['number']}/files?per_page=100&page={page}")
        files += [f["filename"] for f in chunk]
        if len(chunk) < 100: break
        page += 1
    if all(EXEMPT.match(f) for f in files):
        print("docs/CI/meta-only PR: Blender reference not required"); return 0

    body = pr.get("body") or ""
    labels = {l["name"] for l in pr.get("labels", [])}
    # Narrow exemption for pure formatting/lint PRs: label + title prefix + explicit statement, all three required.
    if ("no-behaviour-change" in labels and re.match(r"^(fmt|clippy):", pr.get("title", ""))
            and "NO BEHAVIOUR CHANGE" in body):
        print("fmt/clippy PR with label no-behaviour-change: Blender reference not required.\n"
              "Reviewer: confirm the diff is formatting/lint only and tests are unchanged.")
        return 0
    m = re.search(r"##\s*Blender source ported from\s*(.*?)(?=\n##\s|\Z)", body, re.S | re.I)
    section = m.group(1) if m else ""
    refs = sorted(set(re.findall(r"(?:source|scripts|release|intern|extern|tools|doc)/[\w.\-+/]+", section)))
    refs = [r.rstrip(".,;:)") for r in refs]
    if not refs:
        print("FAIL: the PR body needs a '## Blender source ported from' section listing at least one real\n"
              f"file path from {REF} (e.g. source/blender/editors/mesh/editmesh_select.cc). Do not invent behaviour.")
        return 1
    tree = api(f"https://api.github.com/repos/{REF}/git/trees/{BRANCH}?recursive=1")
    if tree.get("truncated"):
        print("WARN: tree truncated, cannot fully verify")
    paths = {t["path"] for t in tree["tree"]}
    bad = [r for r in refs if r not in paths and not any(p.startswith(r.rstrip("/") + "/") for p in paths)]
    for r in refs:
        print(("MISSING " if r in bad else "ok      ") + r)
    if bad:
        print(f"FAIL: these paths do not exist in {REF}@{BRANCH}. Look them up in the reference repo; never cite from memory.")
        return 1
    return 0

sys.exit(main())
