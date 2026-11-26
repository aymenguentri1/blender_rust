#!/usr/bin/env python3
"""CI gate: PRs must be small and linked to an issue (so a cut-off agent's work can be found and resumed)."""
import json, os, re, sys, urllib.request

MAX_LINES = int(os.environ.get("MAX_PR_LINES", "600"))
IGNORE = re.compile(r"^(Cargo\.lock|docs/|assets/)")

def api(url):
    req = urllib.request.Request(url, headers={"Authorization": "Bearer " + os.environ["GITHUB_TOKEN"],
                                               "Accept": "application/vnd.github+json"})
    return json.load(urllib.request.urlopen(req))

ev = json.load(open(os.environ["GITHUB_EVENT_PATH"]))
pr = ev.get("pull_request")
if not pr:
    print("not a PR"); sys.exit(0)
repo, n = os.environ["GITHUB_REPOSITORY"], pr["number"]
labels = {l["name"] for l in pr.get("labels", [])}
files, page = [], 1
while True:
    chunk = api(f"https://api.github.com/repos/{repo}/pulls/{n}/files?per_page=100&page={page}")
    files += chunk
    if len(chunk) < 100: break
    page += 1
total = sum(f["additions"] + f["deletions"] for f in files if not IGNORE.match(f["filename"]))
code = [f for f in files if not re.match(r"^(\.github/|docs/|scripts/|AGENTS\.md|CLAUDE\.md|README\.md|LICENSE)", f["filename"])]
print(f"changed lines (excluding Cargo.lock/docs/assets): {total}, limit {MAX_LINES}")
fail = False
if total > MAX_LINES and "size-exempt" not in labels:
    print(f"FAIL: PR is too big. Split it into smaller issues/PRs (docs/FAILSAFE.md section 5). "
          "Only the owner may add the label 'size-exempt'.")
    fail = True
if code and not re.search(r"(?:closes|fixes|resolves|part of|resumes)\s+#\d+", pr.get("body") or "", re.I):
    print("FAIL: the PR body must link its issue ('Closes #N', 'Part of #N' or 'Resumes #N') so stale-claim detection "
          "and handoff can find it.")
    fail = True
sys.exit(1 if fail else 0)
