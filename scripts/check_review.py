#!/usr/bin/env python3
"""CI gate `review-gate`: an agent PR needs an approval from a DIFFERENT agent, written after the last push.

Branch naming: agent/<author>/<topic>. Approval = a PR comment, exactly one line:
  REVIEWED agent=<reviewer> VERDICT=APPROVE: compared against <paths or 'n/a docs/ci'>
Human PRs (branch not starting with agent/) are not gated. Author and reviewer names come from the shared GitHub token's
conventions, so this stops accidents (self-merge, stale approval), not a deliberate liar.
"""
import json, os, re, sys, urllib.request
from datetime import datetime

def api(url):
    r = urllib.request.Request(url, headers={"Authorization": "Bearer " + os.environ["GITHUB_TOKEN"], "Accept": "application/vnd.github+json"})
    return json.load(urllib.request.urlopen(r))
def ts(s): return datetime.fromisoformat(s.replace("Z", "+00:00"))

repo, n = os.environ["GITHUB_REPOSITORY"], int(os.environ["PR_NUMBER"])
B = f"https://api.github.com/repos/{repo}"
pr = api(f"{B}/pulls/{n}")
ref = pr["head"]["ref"]
m = re.match(r"^agent/([^/]+)/", ref)
if not m:
    print("not an agent/ branch: review gate not applied"); sys.exit(0)
author = m.group(1)
# update-branch merge commits (1 parent being main) do not change the PR's own work, so they must not invalidate an approval
pcs, pg = [], 1
while True:
    ch = api(f"{B}/pulls/{n}/commits?per_page=100&page={pg}"); pcs += ch
    if len(ch) < 100: break
    pg += 1
own = [c for c in pcs if len(c["parents"]) <= 1]
last_push = max(ts(c["commit"]["committer"]["date"]) for c in (own or pcs))
comments, page = [], 1
while True:
    c = api(f"{B}/issues/{n}/comments?per_page=100&page={page}"); comments += c
    if len(c) < 100: break
    page += 1
pat = re.compile(r"^\s*REVIEWED agent=(\S+) VERDICT=APPROVE: compared against \S+", re.M)
for c in comments:
    for who in pat.findall(c["body"]):
        if who.lower() != author.lower() and ts(c["created_at"]) > last_push:
            print(f"approved by {who} after last push"); sys.exit(0)
print(f"FAIL: needs a comment from an agent other than '{author}', posted AFTER the last push, in exactly this form:\n"
      "REVIEWED agent=<your-name> VERDICT=APPROVE: compared against <Blender paths you opened>\n"
      "Review for real (docs/AGENTS.md section 4 checklist). If the PR is wrong, comment CHANGES REQUESTED instead.")
sys.exit(1)
