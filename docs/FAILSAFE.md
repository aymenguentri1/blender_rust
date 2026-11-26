# Fail-safe protocol: cut-offs, handoffs, small tasks

Any agent can be cut off at any moment (token limit, crash, timeout). This protocol makes that harmless:
work is saved continuously, abandoned work is detected automatically, and another agent resumes it.

## 1. Claim = lease (3 hours, renewed by activity)
Claiming an issue starts a **lease**. Do all three, in this order, within a few minutes:
1. Comment on the issue: `CLAIM agent=<your-name> branch=agent/<your-name>/<topic>`.
2. Add the label `status:claimed` (remove `status:available`).
3. Push the branch and open a **draft PR** whose body contains `Closes #<issue>`.

The lease is renewed by any of: a push to the branch, a `CHECKPOINT` comment, or any comment on the PR.
If nothing happens for **3 hours** the lease expires (see section 3).

## 2. Save work continuously (this is what makes a cut-off cheap)
- **Commit and push after every finished step**, at least every ~15 minutes or ~100 changed lines. Unpushed work is lost work.
  WIP commits are fine: prefix the message `wip:`. The build does not have to pass on a draft branch.
- **Post a CHECKPOINT comment on the draft PR** after each step. Use exactly this shape so a stranger can resume:
  ```
  CHECKPOINT agent=<name> step=<n>/<total>
  DONE: <what works, files touched>
  NEXT: <the very next concrete action>
  BLENDER SOURCE: <paths already read, what is still unread>
  STATE: builds=<yes/no> tests=<yes/no> known problems=<...>
  ```
- Keep the PR body's `## Blender source ported from` list up to date as you go (CI checks it).
- Never keep plans only in your head or in chat. If it is not in a commit or a CHECKPOINT it does not exist.

## 3. Stale detection (automatic)
`.github/workflows/stale-claims.yml` runs every 30 minutes (`scripts/stale_claims.py`). For each open issue labelled
`status:claimed` it takes the newest of: last claim/comment, last commit on the linked branch, last PR activity.
- Older than **3h**: label `status:stale`, remove `status:claimed`, comment that the work can be resumed.
- Stale for another **24h** with no takeover: also remove the assignee and label `status:available`.
A stale issue is **fair game for any agent**. A late original owner who comes back simply continues if nobody took over;
if somebody did, they review the new PR instead.

## 4. Resuming abandoned work (handoff)
1. Pick a `status:stale` issue (prefer these over fresh issues: finishing beats starting).
2. Read the issue, the draft PR, **every CHECKPOINT comment** and `git log origin/<old-branch>`.
3. Comment `TAKEOVER agent=<you>` on the issue, swap labels to `status:claimed`.
4. **Branch from the abandoned branch, never commit to it** (this is the one exception to "do not use another agent's
   branch": you may *fork* a stale branch):
   `git fetch origin && git checkout -b agent/<you>/resume-<issue>-<topic> origin/<old-branch>`
5. Run `cargo build --all-targets && cargo test`, read the diff against `main`, and re-check it against the cited Blender
   files (WIP code from a cut-off agent is untrusted until you have verified it).
6. Open a new draft PR (`Closes #<issue>`, `Resumes #<old-PR>`), then close the old PR with a comment linking the new one.
   Never delete the old branch; it is the audit trail.
7. Continue with section 2.

## 5. Keep tasks small so cut-offs are rare
- One issue = one PR = one Blender source file (or one function group) = **size S or M**:
  - `size:S`: up to ~150 changed lines, 1-2 files touched.
  - `size:M`: up to ~400 changed lines, at most ~4 files touched.
- Larger work is an `epic`: a tracking issue with a checklist of S/M child issues. **Never claim an epic.**
- If your task turns out bigger than M, **stop**, push what you have, and post a CHECKPOINT proposing the split. Do not push on.
- CI enforces the cap (`pr-size`): above 600 changed lines (ignoring `Cargo.lock`, `docs/`, `assets/`) the check fails
  unless the PR carries the label `size-exempt`, which only the owner (aymenguentri1) may add.
- Work on **one issue at a time**. Finish or hand off before claiming another.

## 6. Proof that nothing is broken (what gates every merge)
`build-test` (build and tests), `blender-ref` (cited Blender files exist), `pr-size` (size cap and issue link).
All three are required on `main`. CI cannot prove behaviour matches Blender, so a reviewer must open the cited Blender
files and compare before approving; the reviewer writes `REVIEWED agent=<name>: compared against <paths>` in the PR.
