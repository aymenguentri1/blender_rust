#!/usr/bin/env python3
"""Factory loop for many Claudes. Stdlib only. Env: GH_TOKEN, AGENT_NAME. Run from the blender_rust clone.

  next                         what to do now (priority: fix main > my PRs > merge > review > takeover > claim > plan)
  start N [--topic T]          claim issue N (race-safe), branch, draft PR   (stale issue => takeover automatically)
  checkpoint PR --step 2/5 --done ".." --next ".." [--state ".."]   push + CHECKPOINT comment
  check                        cargo build+test, prints only OK or the failure tail
  ready PR                     mark ready for review
  diff PR [--max 800]          cited Blender paths + diff of a PR (for reviewers)
  review PR --paths a,b | --changes "text"   approve (different agent only) or request changes
  merge PR [--wait 40]         update branch, wait for CI, squash-merge (no tokens spent waiting)
  plan-next                    next unplanned roadmap area + candidate Blender files
  new-issues spec.json         validated, de-duplicated creation of an epic + S/M children
"""
import argparse, base64, json, os, re, subprocess, sys, time, urllib.request, urllib.error
REPO = os.environ.get("FACTORY_REPO", "aymenguentri1/blender_rust")
TOKEN = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN") or ""
REQUIRED = ["build-test", "blender-ref", "pr-size", "review-gate"]
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
REFDIR = os.environ.get("BLENDER_REF_DIR", os.path.join(os.path.dirname(ROOT), "blender"))
MIN_AVAILABLE = 15

def die(msg, code=1): print(msg); sys.exit(code)
def finish(msg): print(msg); sys.exit(0)
def call(m, path, data=None):
    if not TOKEN: die("set GH_TOKEN")
    url = path if path.startswith("http") else f"https://api.github.com/repos/{REPO}{path}"
    req = urllib.request.Request(url, method=m, data=json.dumps(data).encode() if data is not None else None,
        headers={"Authorization": "Bearer " + TOKEN, "Accept": "application/vnd.github+json", "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req) as r:
            b = r.read(); return r.status, (json.loads(b) if b else None)
    except urllib.error.HTTPError as e:
        b = e.read().decode()
        try: return e.code, json.loads(b)
        except Exception: return e.code, {"message": b[:300]}
def api(m, path, data=None):
    c, b = call(m, path, data)
    if c >= 300: die(f"API {m} {path} -> {c}: {(b or {}).get('message')}")
    return b
def pages(path):
    out, p = [], 1
    sep = "&" if "?" in path else "?"
    while True:
        chunk = api("GET", f"{path}{sep}per_page=100&page={p}")
        out += chunk
        if len(chunk) < 100: return out
        p += 1
def me(a):
    n = getattr(a, "me", None) or os.environ.get("AGENT_NAME", "")
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{2,30}", n or ""): die("set AGENT_NAME (lowercase letters, digits, dashes; e.g. claude-k3x9)")
    return n
def git(*args, check=True, cwd=ROOT):
    h = "Authorization: basic " + base64.b64encode(f"x-access-token:{TOKEN}".encode()).decode()
    r = subprocess.run(["git", "-c", "http.extraheader=" + h, *args], cwd=cwd, capture_output=True, text=True)
    if check and r.returncode: die(f"git {args[0]} failed: {(r.stderr or r.stdout)[-300:].replace(TOKEN, '***')}")
    return r.stdout
def ts(s): return time.mktime(time.strptime(s, "%Y-%m-%dT%H:%M:%SZ"))
def comments(n): return pages(f"/issues/{n}/comments")
def author_of(pr):
    m = re.match(r"agent/([^/]+)/", pr["head"]["ref"]); return m.group(1) if m else None
def own_commit_time(n):
    cs = [c for c in pages(f"/pulls/{n}/commits") if len(c["parents"]) <= 1]   # update-branch merge commits don't count
    return max(ts(c["commit"]["committer"]["date"]) for c in cs) if cs else 0
def checks(sha):
    best = {}
    for c in api("GET", f"/commits/{sha}/check-runs?per_page=100")["check_runs"]:
        if c["name"] not in best or c["id"] > best[c["name"]][0]: best[c["name"]] = (c["id"], c["conclusion"] or c["status"])
    return {k: v[1] for k, v in best.items()}
def labels(i): return {l["name"] for l in i["labels"]}
def cited_paths(text):
    return sorted(set(x.rstrip(".,;:)") for x in re.findall(r"`((?:source|scripts|release|intern|extern|tools|doc)/[\w.\-+/]+)`", text or "")))
def changes_pending(pr, me_commit_t=None):
    cs = comments(pr["number"]); last = [c for c in cs if re.match(r"\s*CHANGES REQUESTED", c["body"])]
    if not last: return None
    t = me_commit_t if me_commit_t is not None else own_commit_time(pr["number"])
    return last[-1]["body"] if ts(last[-1]["created_at"]) > t else None

# ---------------------------------------------------------------- next
def cmd_next(a):
    name = me(a)
    c, runs = call("GET", "/actions/workflows/ci.yml/runs?branch=main&event=push&status=completed&per_page=1")
    if c == 200 and runs["workflow_runs"] and runs["workflow_runs"][0]["conclusion"] == "failure":
        finish(f"FIX MAIN: CI on main is red ({runs['workflow_runs'][0]['html_url']}). Find the culprit merge, revert it via a PR (branch agent/{name}/revert-..., "
            "title 'fmt:'-free, cite the same Blender files), merge it, then fix forward. Nothing else matters until main is green.")
    prs = [p for p in pages("/pulls?state=open") if p["head"]["repo"]["full_name"] == REPO]
    mine = [p for p in prs if author_of(p) == name]
    for p in mine:
        if p["draft"]: continue
        st = checks(p["head"]["sha"]); bad = [k for k in ("build-test", "blender-ref", "pr-size") if st.get(k) == "failure"]
        ch = changes_pending(p)
        if bad or ch: finish(f"FIX #{p['number']}: {'CI red: ' + ','.join(bad) if bad else 'reviewer asked for changes: ' + ch[:300]}. Fix on your own branch and push; reviewers re-check after your push.")
    for p in mine:
        if p["draft"]: finish(f"CONTINUE #{p['number']} ({p['title']}): your open draft PR. Read its CHECKPOINT comments, keep going, `factory.py checkpoint` after each step, `factory.py check` then `factory.py ready {p['number']}` when done.")
    ready = [p for p in prs if not p["draft"]]
    cache = {p["number"]: checks(p["head"]["sha"]) for p in ready}
    for p in sorted(ready, key=lambda x: x["number"]):
        if all(cache[p["number"]].get(r) == "success" for r in REQUIRED): finish(f"MERGE #{p['number']}: `python3 scripts/factory.py merge {p['number']}` (approved and green).")
    for p in sorted(ready, key=lambda x: x["number"]):
        if author_of(p) in (name, None): continue
        st = cache[p["number"]]
        if st.get("review-gate") == "success" or st.get("build-test") == "failure" or changes_pending(p): continue
        finish(f"REVIEW #{p['number']} ({p['title']}): `python3 scripts/factory.py diff {p['number']}`, open the cited Blender files, compare, `cargo test` if unsure, then "
            f"`factory.py review {p['number']} --paths <blender paths you opened>` or `--changes \"<specifics>\"`.")
    issues = [i for i in pages("/issues?state=open&labels=status:stale") if "pull_request" not in i]
    if issues: finish(f"TAKEOVER #{issues[0]['number']} ({issues[0]['title']}): `python3 scripts/factory.py start {issues[0]['number']}` (resumes the abandoned branch).")
    av = [i for i in pages("/issues?state=open&labels=status:available") if "pull_request" not in i and "epic" not in labels(i)]
    def key(i): return ("## Order" in (i["body"] or ""), 0 if "size:S" in labels(i) else 1, i["number"])
    av.sort(key=key)
    if av:
        i = av[0]; order = re.search(r"## Order\n(.*)", i["body"] or "")
        finish(f"CLAIM #{i['number']} ({i['title']}){'  NOTE: ' + order.group(1) if order else ''}: `python3 scripts/factory.py start {i['number']}`. Then read only the cited Blender files.")
    finish("PLAN: backlog is empty. `python3 scripts/factory.py plan-next`")

# ---------------------------------------------------------------- start
def cmd_start(a):
    name = me(a); n = a.issue; i = api("GET", f"/issues/{n}")
    if i["state"] != "open" or "pull_request" in i: die("not an open issue")
    L = labels(i)
    if "epic" in L: die("epics cannot be claimed; claim a child issue")
    stale = "status:stale" in L
    if not (stale or "status:available" in L): die(f"#{n} is not available (labels: {sorted(L)}); run `factory.py next`")
    slug = re.sub(r"[^a-z0-9]+", "-", (a.topic or i["title"]).lower()).strip("-")[:40]
    old = None
    if stale:
        for p in pages("/pulls?state=open"):
            if re.search(rf"(closes|fixes|resolves|part of|resumes)\s+#{n}\b", p["body"] or "", re.I): old = p; break
    kind = "TAKEOVER" if stale else "CLAIM"; branch = f"agent/{name}/{'resume-' + str(n) + '-' if old else ''}{slug}"
    c = api("POST", f"/issues/{n}/comments", {"body": f"{kind} agent={name} branch={branch}"})
    cs = comments(n); start = 0
    for k, x in enumerate(cs):
        if x["user"]["login"].endswith("[bot]") and re.match(r"(STALE|RELEASED):", x["body"]): start = k + 1
    claims = [x for x in cs[start:] if re.match(r"\s*(CLAIM|TAKEOVER) agent=", x["body"])]
    win = min(claims, key=lambda x: (x["created_at"], x["id"]))
    if win["id"] != c["id"]:
        call("DELETE", f"/issues/comments/{c['id']}"); die(f"TAKEN: another agent claimed #{n} first. Run `factory.py next`.", 5)
    for l in ("status:available", "status:stale"):
        call("DELETE", f"/issues/{n}/labels/{urllib.request.quote(l)}")
    api("POST", f"/issues/{n}/labels", {"labels": ["status:claimed"]})
    git("fetch", "-q", "origin"); base = f"origin/{old['head']['ref']}" if old else "origin/main"
    if old: git("fetch", "-q", "origin", old["head"]["ref"])
    git("checkout", "-q", "-b", branch, base)
    git("-c", f"user.name={name}", "-c", "user.email=noreply@anthropic.com", "commit", "-q", "--allow-empty", "-m", f"wip: {'resume' if old else 'start'} #{n}")
    git("push", "-q", "origin", branch)
    refs = cited_paths(i["body"]); body = (f"Closes #{n}\n" + (f"Resumes #{old['number']}\n" if old else "") +
        "\n## Blender source ported from\n" + ("\n".join(f"- `{r}`" for r in refs) or "- `source/blender/...`  <!-- replace with real paths -->") +
        "\n\n(Keep this list to the files you actually used. CI verifies them. Progress is in CHECKPOINT comments.)\n")
    pr = api("POST", "/pulls", {"title": i["title"], "head": branch, "base": "main", "body": body, "draft": True})
    if old:
        api("POST", f"/issues/{old['number']}/comments", {"body": f"Superseded by #{pr['number']} (takeover by {name}). Branch kept as audit trail."})
        api("PATCH", f"/pulls/{old['number']}", {"state": "closed"})
    print(f"OK {kind.lower()} #{n}: branch {branch}, draft PR #{pr['number']}. Blender files to read first: {', '.join(refs) or 'see issue'}. "
          f"Next: implement in small steps; `factory.py checkpoint {pr['number']} --step 1/N --done .. --next ..` after each.")

def cmd_checkpoint(a):
    name = me(a); git("push", "-q", "origin", "HEAD")
    body = (f"CHECKPOINT agent={name} step={a.step}\nDONE: {a.done}\nNEXT: {a.next}\nSTATE: {a.state or 'n/a'}")
    api("POST", f"/issues/{a.pr}/comments", {"body": body}); print("pushed + checkpoint posted")

def cmd_check(a):
    for cmd in (["cargo", "build", "--all-targets", "-q"], ["cargo", "test", "-q"]):
        try: r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        except FileNotFoundError: die("cargo not found: install the Rust toolchain (rustup) or rely on CI; do not skip `check` silently", 4)
        if r.returncode: die("FAILED " + " ".join(cmd) + "\n" + "\n".join((r.stdout + r.stderr).splitlines()[-40:]), 4)
    print("OK build + test")

def cmd_ready(a):
    pr = api("GET", f"/pulls/{a.pr}")
    if not pr["draft"]: print("already ready"); return
    q = "mutation($id:ID!){markPullRequestReadyForReview(input:{pullRequestId:$id}){pullRequest{number}}}"
    c, b = call("POST", "https://api.github.com/graphql", {"query": q, "variables": {"id": pr["node_id"]}})
    if c != 200 or (b or {}).get("errors"): die(f"ready failed: {b}")
    print("ready for review")

def cmd_diff(a):
    pr = api("GET", f"/pulls/{a.pr}"); print(f"#{a.pr} {pr['title']}  author={author_of(pr)}  checks={checks(pr['head']['sha'])}")
    print("Cited Blender paths:", ", ".join(cited_paths(pr["body"])) or "NONE")
    git("fetch", "-q", "origin", f"pull/{a.pr}/head:refs/factory/pr-{a.pr}", "-f"); git("fetch", "-q", "origin", "main")
    d = git("diff", "origin/main...refs/factory/pr-%d" % a.pr).splitlines()
    print("\n".join(d[:a.max])); 
    if len(d) > a.max: print(f"... {len(d) - a.max} more lines (git diff origin/main...refs/factory/pr-{a.pr})")

def cmd_review(a):
    name = me(a); pr = api("GET", f"/pulls/{a.pr}"); au = author_of(pr)
    if au == name: die("you cannot review your own PR")
    if a.changes: body = f"CHANGES REQUESTED agent={name}: {a.changes}"
    else:
        if not a.paths: die("approve needs --paths <blender files you actually opened>")
        body = f"REVIEWED agent={name} VERDICT=APPROVE: compared against {a.paths.replace(' ', '')}"
    api("POST", f"/issues/{a.pr}/comments", {"body": body}); print("posted:", body[:120])

def cmd_merge(a):
    n = a.pr; end = time.time() + a.wait * 60; last = ""
    while True:
        pr = api("GET", f"/pulls/{n}")
        if pr["merged"]: print("merged"); return
        if pr["state"] != "open": die("PR closed")
        if pr["draft"]: die("PR is a draft: `factory.py ready` first")
        st = checks(pr["head"]["sha"]); sha = pr["head"]["sha"]; ms = pr["mergeable_state"]
        fails = [k for k in REQUIRED if st.get(k) == "failure"]
        if "review-gate" in fails and all(st.get(k) == "success" for k in REQUIRED if k != "review-gate"): die("NEEDS REVIEW: no approval from another agent after the last push", 2)
        if [f for f in fails if f != "review-gate"]: die(f"CI FAILED: {[f for f in fails if f != 'review-gate']}. Fix and push.", 4)
        if ms == "dirty": die("CONFLICT with main: `git fetch && git rebase origin/main`, resolve, `git push --force-with-lease` (your own branch only), ask for re-review.", 3)
        if ms == "behind":
            c, b = call("PUT", f"/pulls/{n}/update-branch", {"expected_head_sha": sha})
            if c == 422 or c == 409: die("CONFLICT while updating branch: rebase onto origin/main yourself.", 3)
        elif all(st.get(k) == "success" for k in REQUIRED) and ms in ("clean", "has_hooks", "unstable"):
            c, b = call("PUT", f"/pulls/{n}/merge", {"merge_method": "squash", "sha": sha})
            if c == 200: print("merged"); return
            last = f"merge returned {c}: {(b or {}).get('message')}"
        if time.time() > end: die(f"timeout waiting ({ms}, {st}) {last}", 6)
        time.sleep(20)

# ---------------------------------------------------------------- planning
def ref_paths():
    if not os.path.isdir(REFDIR): die("run scripts/blender_ref.sh setup first")
    return set(subprocess.run(["git", "-C", REFDIR, "ls-tree", "-r", "--name-only", "HEAD"], capture_output=True, text=True).stdout.split("\n"))
def norm(t): return set(re.findall(r"[a-z0-9]+", t.lower()))
def cmd_plan_next(a):
    road = json.load(open(os.path.join(ROOT, "docs/roadmap.json")))
    allissues = [i for i in pages("/issues?state=all") if "pull_request" not in i]
    covered = {m for i in allissues if "epic" in labels(i) for m in re.findall(r"roadmap-key: ([\w-]+)", i["body"] or "")}
    avail = len([i for i in allissues if i["state"] == "open" and "status:available" in labels(i) and "epic" not in labels(i)])
    todo = [r for r in road if r["key"] not in covered]
    print(f"available backlog: {avail}. Unplanned areas: {len(todo)}/{len(road)}.")
    if avail >= 40: die("Backlog is full (>=40 available). Do not plan; take a task via `factory.py next`.")
    if not todo: die("Roadmap fully planned. Add areas to docs/roadmap.json (docs-only PR) from Blender's source tree.")
    r = todo[0]; used = {p for i in allissues for p in cited_paths(i["body"])}
    paths = ref_paths(); cand = sorted(p for p in paths if any(p.startswith(d) for d in r["dirs"]) and re.search(r"\.(cc|c|h|hh|py|cpp)$", p) and "_test." not in p and p not in used)
    print(f"AREA {r['key']}: {r['title']}\nNOTE: {r.get('note', '')}\nCandidate Blender files not yet cited ({len(cand)}), first 50:")
    print("\n".join(cand[:50]))
    print("\nTo plan: read the few most central files (skim with scripts/blender_ref.sh show <path> 1 80), then write spec.json:\n"
          '{"epic":{"title":"..","what":"..","areas":["area:ui"],"key":"%s"},"children":[{"size":"S|M","title":"..","what":"..","paths":["real/path"],'
          '"acceptance":"testable","order":"optional dependency note"}]}\nRules: 3-8 children, one source file (or function group) each, size S (<=150 lines) or M (<=400), tests required.\n'
          "Then `python3 scripts/factory.py new-issues spec.json` (validates paths, rejects duplicates)." % r["key"])

TEMPLATE_RULES = "**Rules:** follow `docs/QUICK.md`. Run `python3 scripts/factory.py start <this issue>`; read only the cited Blender files; one small step at a time, checkpoint after each."
def cmd_new_issues(a):
    spec = json.load(open(a.spec)); paths = ref_paths(); ep = spec.get("epic") or {}; kids = spec.get("children", [])
    if not (2 <= len(kids) <= 10): die("need 2-10 children")
    errs = []
    for k in kids:
        if k.get("size") not in ("S", "M"): errs.append(f"size must be S or M: {k.get('title')}")
        if not k.get("paths"): errs.append(f"no Blender paths: {k.get('title')}")
        for p in k.get("paths", []):
            if p not in paths: errs.append(f"path does not exist in reference repo: {p}")
        if not k.get("acceptance"): errs.append(f"no acceptance criteria: {k.get('title')}")
    existing = [i for i in pages("/issues?state=all") if "pull_request" not in i]
    for t in [ep.get("title", "")] + [k["title"] for k in kids]:
        for i in existing:
            x, y = norm(t), norm(i["title"])
            if x and y and len(x & y) / len(x | y) >= 0.7: errs.append(f"duplicate of #{i['number']}: {t}")
    if errs: die("REJECTED:\n- " + "\n- ".join(errs))
    e = api("POST", "/issues", {"title": ep["title"], "labels": ep.get("areas", []) + ["epic"], "body": ep["what"] + f"\n\nroadmap-key: {ep['key']}\n"})
    made = []
    for k in kids:
        body = (f"Part of epic #{e['number']}. Size: **{k['size']}**.\n\n## What\n{k['what']}\n\n## Blender source (verified to exist)\n" + "\n".join(f"- `{p}`" for p in k["paths"]) +
                f"\n\n## Acceptance criteria\n{k['acceptance']}\n" + (f"\n## Order\n{k['order']}\n" if k.get("order") else "") + "\n" + TEMPLATE_RULES + "\n")
        i = api("POST", "/issues", {"title": k["title"], "body": body, "labels": ep.get("areas", []) + [f"size:{k['size']}", "status:available"]}); made.append(i); time.sleep(1)
    api("PATCH", f"/issues/{e['number']}", {"body": ep["what"] + f"\n\nroadmap-key: {ep['key']}\n\n---\n**EPIC: do not claim; claim a child:**\n" + "\n".join(f"- [ ] #{i['number']} {i['title']} ({k['size']})" for i, k in zip(made, kids))})
    print(f"created epic #{e['number']} with children {[i['number'] for i in made]}")

if __name__ == "__main__":
    p = argparse.ArgumentParser(); s = p.add_subparsers(dest="cmd", required=True)
    def add(name, fn, *args, **kw):
        sp = s.add_parser(name); sp.add_argument("--me")
        for a_, k_ in args: sp.add_argument(a_, **k_)
        sp.set_defaults(fn=fn); return sp
    add("next", cmd_next); add("start", cmd_start, ("issue", {"type": int}), ("--topic", {}))
    add("checkpoint", cmd_checkpoint, ("pr", {"type": int}), ("--step", {"required": True}), ("--done", {"required": True}), ("--next", {"required": True}), ("--state", {}))
    add("check", cmd_check); add("ready", cmd_ready, ("pr", {"type": int})); add("diff", cmd_diff, ("pr", {"type": int}), ("--max", {"type": int, "default": 800}))
    add("review", cmd_review, ("pr", {"type": int}), ("--paths", {}), ("--changes", {})); add("merge", cmd_merge, ("pr", {"type": int}), ("--wait", {"type": int, "default": 40}))
    add("plan-next", cmd_plan_next); add("new-issues", cmd_new_issues, ("spec", {}))
    args = p.parse_args(); args.fn(args)
