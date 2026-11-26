# AGENTS.md - rules for every Claude (and human) working on blender_rust

Goal: a full Blender clone in Rust. **Start with `docs/QUICK.md` (1 minute) and run `python3 scripts/factory.py next`; read the rest of this file only when a rule is unclear.**
`CLAUDE.md` points here. These rules apply to every agent equally.

## 0. Golden rules (never break these)
1. **Never push directly to `main`.** Always work on a branch and merge through a PR.
2. **Never force-push, never rewrite history on a shared branch, never delete `main`.**
3. **One agent per branch.** Never commit to a branch another agent created. (Exception: you may *fork* a branch whose issue is
   `status:stale`, see `docs/FAILSAFE.md` section 4. You still never push to it.)
4. **Never merge a PR whose CI is red.** Never disable, skip, or weaken CI to get a merge.
5. **Never leave `main` broken.** `cargo build` and `cargo test` must pass on `main` at all times.
6. **If unsure, stop and open an issue / ask the owner (aymenguentri1).** Do not guess on design.
7. **Never print, commit, or store tokens/secrets.** Check `git diff` before each commit.

## 1. Source of truth: the official Blender repo
- Reference: https://github.com/aymenguentri1/blender (mirror of official Blender). Clone it read-only
  into a folder OUTSIDE this repo (e.g. `../blender`). **Never push to it. Never add it as a submodule or commit it here.**
- Port behaviour from Blender: algorithms, defaults, key bindings, operator names, UI layout, and data structures
  must match Blender. Before implementing a feature, find the Blender source for it
  (`source/blender/...`, `scripts/startup/bl_ui/...`, `release/datafiles/...`) and follow it.
- **Do not invent features** that Blender does not have, and do not invent behaviour. If Blender does X, do X.
  Where we deviate (simplification, missing feature), document it in the README "Deviations" text and `docs/DECISIONS.md`.
- Cite the Blender source file you ported in a comment or the PR description (`Ported from source/blender/.../file.c`).
- Do not add third-party code, assets, or algorithms from anywhere else. Dependencies need approval (see section 6).

### Mandatory reference procedure (no exceptions)
Never write Blender behaviour from memory. For every feature, fix or UI element:
1. `scripts/blender_ref.sh setup` (clones https://github.com/aymenguentri1/blender read-only next to this repo).
2. Find the real source: `scripts/blender_ref.sh find <name>` and `scripts/blender_ref.sh grep <text>`.
   UI layout and labels come from `scripts/startup/bl_ui/*.py`; operators and behaviour from
   `source/blender/editors/**`; data and algorithms from `source/blender/blenkernel`, `bmesh`, `modifiers`, `draw`, `gpu`.
3. Read the actual code. Copy exact names, defaults, constants, key maps and ordering from it.
4. If you cannot find it in the reference repo, **it does not exist: do not build it.** Open an issue instead.
5. List every file you used under `## Blender source ported from` in the PR body (full repo-relative paths).
   The `blender-ref` CI check verifies each path exists in the reference repo and **blocks the merge** if the section is
   missing or a path is wrong. Reviewers must also open the cited files and confirm the Rust matches them.
6. Any difference from Blender goes in `docs/DECISIONS.md` and the README deviations, with the reason.
Doc/CI/meta-only PRs are exempt from the CI check, never from rule 4.

## 2. Licensing (decided, do not change)
Blender is GPL-2.0-or-later. Because we port its code, **this project is licensed GPL-3.0-or-later**
(compatible with Blender's "or later"). Keep Blender's copyright credit in files that are direct ports.
Never paste code from non-GPL-compatible sources.

## 3. Workflow for every task
1. Pick or create a GitHub Issue. **Claim it** (assign yourself or comment "claimed by <agent-name>"). Check
   open issues and open PRs first: if someone already covers it, choose another task.
2. `git fetch origin && git checkout -b agent/<name>/<short-topic> origin/main`
   (example: `agent/claude-a/sculpt-face-sets`).
3. **Open a Draft PR immediately.** This is the "I am working here" signal for everyone else.
4. Work in small commits and **push after every finished step** (at least every ~15 minutes). Keep PRs **size S or M**
   (under ~400 changed lines; CI fails above 600). Split larger work.
5. Rebase on `origin/main` often (`git fetch && git rebase origin/main`). Resolve conflicts carefully;
   never discard another agent's work to make a conflict go away.
6. Before marking ready: run the checks in section 5 locally if a toolchain is available.
7. Mark the PR ready, fill in the PR template, wait for CI green.
8. Merge rules in section 4.

## 4. Review and merge
- **Squash-merge only.** Delete the branch after merge.
- **Automatic review and merge (no owner step needed).** Roles: the *author* opens and finishes the PR; a *different* agent is the
  *reviewer*. The reviewer opens the cited Blender files, compares the Rust against them, runs `cargo test`, then comments exactly:
  `REVIEWED agent=<you> VERDICT=APPROVE: compared against <paths>` (or `CHANGES REQUESTED: <specifics>`). The `review-gate` check
  accepts only an approval from another agent posted after the last push. When all required checks are green, **anyone (author or
  reviewer) merges** with `gh pr merge <n> --squash --delete-branch` (or `--auto` to merge when checks finish).
- **Review duty:** before claiming new work, review one waiting ready PR that is not yours (oldest first). Reviewing is part of the job.
- An agent may merge a PR **only if all are true**: CI is green; the PR is not its own work
  *or* a second agent/human has reviewed it (the `review-gate` check enforces this for every `agent/` branch);
  the branch is up to date with `main`; no unresolved review comments; the PR matches Blender's behaviour per section 1.
- **Reviewer checklist:** compiles and tests pass; matches the cited Blender source; no invented behaviour;
  no unrelated changes; no new dependency without approval; no `unsafe` without a justification comment;
  no `unwrap()`/`panic!` on user-driven paths; tests added for new logic; README updated for user-visible features.
- Review comments must be specific. Fix review comments on the same branch.
- If two PRs conflict, the one merged second rebases. Do not merge both and "fix later".

## 5. Quality gates (CI runs these)
```
cargo build --all-targets          # blocking
cargo test                         # blocking
cargo fmt --check                  # warning for now, will become blocking
cargo clippy -- -D warnings        # warning for now, will become blocking
```
New code should be fmt- and clippy-clean already. Do not reformat files you are not otherwise changing
(it creates conflicts for everyone).

## 6. Architecture and conflict avoidance
- Crate layout is a single crate today (`src/*.rs`). Prefer **adding new files/modules** over growing the
  big shared files. Hotspots that cause conflicts: `src/main.rs`, `src/scene.rs`, `src/view3d.rs`,
  `src/scene/ops.rs`. Touch them minimally (register a module, add a call) and merge those PRs fast.
- Before editing a hotspot file, check open PRs touching it (`gh pr list` / `git diff origin/main...origin/<branch> --stat`).
- Follow existing naming: files named after the Blender area they port (`view3d`, `outliner`, `sculpt`, ...).
- Adding a dependency to `Cargo.toml` needs the owner's approval in the issue first. Prefer the standard library
  (the project is deliberately near dependency-free).
- Performance matters: do not regress `bench_draw_scene`.

## 7. Ownership and areas
One agent owns an area at a time; others contribute via PR to that area only after checking open PRs.
Record who currently works on what in `docs/TASKS.md` (update it in your PR when you claim/finish an area).

## 8. Records
- `docs/DECISIONS.md`: every design decision and every deliberate deviation from Blender, with the Blender
  source reference. Read it before re-deciding anything.
- `docs/TASKS.md`: roadmap and who is on what.

## 9. When something goes wrong
- Broken `main`: the agent whose merge caused it reverts it **immediately** (`git revert`, via PR), then fixes forward.
- Never "fix" by force-pushing. Never delete other agents' branches.
- Tell the owner in the PR/issue what happened.

## 10. Fail-safe: cut-offs, leases and handoff (read `docs/FAILSAFE.md`, it is mandatory)
You can be cut off at any time (token limit, crash). So:
- **Claiming is a 3-hour lease**: claim comment + label `status:claimed` + draft PR with `Closes #N`. Activity renews it.
- **Push early, push often, and post a `CHECKPOINT` comment on your draft PR after every step** (format in `docs/FAILSAFE.md`).
  Unpushed work is lost work. Plans kept only in chat are lost plans.
- A bot marks claims with no activity for 3h as `status:stale`. **Prefer resuming a `status:stale` issue over starting
  a new one**: comment `TAKEOVER`, branch from the old branch, re-verify it against the Blender source, open a new draft PR.
- Only claim `size:S` or `size:M` issues. Never claim an `epic`. If your task grows beyond M, stop and propose a split.
- One issue at a time. Finish or hand off before taking another.
- Required checks for merging: `build-test`, `blender-ref`, `pr-size`, `review-gate`.
- Formatting/lint-only PRs (issues under epic #75): title starts `fmt:` or `clippy:`, label `no-behaviour-change`, and the body
  states `NO BEHAVIOUR CHANGE`. Only then is the `## Blender source ported from` section not required. Reviewers confirm the diff
  really is formatting/lint only. The fmt-everything PR also needs the owner to add `size-exempt` (it exceeds the size cap).
