#!/usr/bin/env python3
"""Mark claimed issues stale when the claiming agent went silent (cut off, crashed, out of tokens).

Run by .github/workflows/stale-claims.yml every 30 minutes. Lease: LEASE_H hours of no activity.
Env: GITHUB_TOKEN, GITHUB_REPOSITORY. Optional: DRY_RUN=1, LEASE_H, RELEASE_H.
"""
import json, os, re, sys, urllib.request, urllib.error
from datetime import datetime, timezone

REPO = os.environ["GITHUB_REPOSITORY"]
TOKEN = os.environ["GITHUB_TOKEN"]
LEASE_H = float(os.environ.get("LEASE_H", "3"))
RELEASE_H = float(os.environ.get("RELEASE_H", "24"))
DRY = os.environ.get("DRY_RUN") == "1"
API = f"https://api.github.com/repos/{REPO}"

def call(method, path, data=None):
    req = urllib.request.Request(API + path if path.startswith("/") else path, method=method,
        data=json.dumps(data).encode() if data is not None else None,
        headers={"Authorization": "Bearer " + TOKEN, "Accept": "application/vnd.github+json"})
    try:
        with urllib.request.urlopen(req) as r:
            body = r.read()
            return json.loads(body) if body else None
    except urllib.error.HTTPError as e:
        if e.code == 404 and method == "DELETE":
            return None
        raise

def ts(s): return datetime.fromisoformat(s.replace("Z", "+00:00"))
def hours(t): return (datetime.now(timezone.utc) - t).total_seconds() / 3600

def issues(label):
    out, page = [], 1
    while True:
        chunk = call("GET", f"/issues?state=open&labels={label}&per_page=100&page={page}")
        out += [i for i in chunk if "pull_request" not in i]
        if len(chunk) < 100: return out
        page += 1

def last_activity(issue):
    """Newest of: issue update/comments, linked PR activity, linked branch commits."""
    times = [ts(issue["created_at"])]   # not updated_at: our own label/comment edits must not renew a lease
    for c in call("GET", f"/issues/{issue['number']}/comments?per_page=100"):
        if "github-actions" not in c["user"]["login"]:      # our own stale notices are not activity
            times.append(ts(c["created_at"]))
    for pr in call("GET", "/pulls?state=open&per_page=100"):
        if re.search(rf"(?:closes|fixes|resolves|part of|resumes)\s+#{issue['number']}\b", pr.get("body") or "", re.I):
            times.append(ts(pr["updated_at"]))
            try:
                commits = call("GET", f"/commits?sha={pr['head']['ref']}&per_page=1")
                times.append(ts(commits[0]["commit"]["committer"]["date"]))
            except Exception:
                pass
            for c in call("GET", f"/issues/{pr['number']}/comments?per_page=100"):
                if "github-actions" not in c["user"]["login"]:
                    times.append(ts(c["created_at"]))
    return max(times)

def comment(n, text):
    print(f"#{n}: {text.splitlines()[0]}")
    if not DRY: call("POST", f"/issues/{n}/comments", {"body": text})

def swap(n, remove, add):
    if DRY: return
    for l in remove: call("DELETE", f"/issues/{n}/labels/{urllib.request.quote(l)}")
    if add: call("POST", f"/issues/{n}/labels", {"labels": add})

def main():
    for i in issues("status:claimed"):
        age = hours(last_activity(i))
        if age >= LEASE_H:
            comment(i["number"],
                f"STALE: no activity for {age:.1f}h (lease is {LEASE_H:g}h). The previous claimant was probably cut off.\n\n"
                "Any agent may resume this: follow `docs/FAILSAFE.md` section 4 (read the draft PR and its CHECKPOINT comments, "
                "comment `TAKEOVER agent=<you>`, branch from the old branch, open a new draft PR). "
                "If the original agent is back, comment on the issue or push to renew the lease.")
            swap(i["number"], ["status:claimed"], ["status:stale"])
    for i in issues("status:stale"):
        age = hours(last_activity(i))
        if age >= LEASE_H + RELEASE_H:
            comment(i["number"], f"RELEASED: stale for {age:.0f}h with no takeover. Unassigned and marked available.")
            if not DRY:
                for a in i["assignees"]:
                    call("DELETE", f"/issues/{i['number']}/assignees", {"assignees": [a["login"]]})
            swap(i["number"], ["status:stale"], ["status:available"])
    return 0

sys.exit(main())
