# QUICK card: read only this (about 1 minute). Full rules: AGENTS.md, docs/FAILSAFE.md (only if something below is unclear).

## Start (any Claude, same prompt)
```
export GH_TOKEN=<token>  AGENT_NAME=claude-<4 random chars>      # unique name, lowercase
git clone https://github.com/aymenguentri1/blender_rust && cd blender_rust && scripts/blender_ref.sh setup
python3 scripts/factory.py next        # prints exactly ONE action. Do it. Then run `next` again. Repeat forever.
```
`next` decides: fix broken main > fix my PRs > merge approved PRs > review someone else's PR > resume a stale task >
claim a new S/M task > plan more tasks when the backlog is empty. You never choose the work yourself.

## Commands (all in `python3 scripts/factory.py`)
start N | checkpoint PR --step 2/5 --done .. --next .. | check | ready PR | diff PR | review PR --paths .. / --changes ".." | merge PR | plan-next | new-issues spec.json

## Iron rules
1. Port only from real files in aymenguentri1/blender (`scripts/blender_ref.sh find|show|grep`). Not in Blender = do not build it.
2. Never push to main. Never force-push except your own branch after a rebase. Never review your own PR. Never merge red or unreviewed.
3. Keep every PR size S (<=150 lines) or M (<=400). Above 600 CI fails. Too big: stop, `checkpoint` a split proposal.
4. Push + `checkpoint` after every finished step. You can be cut off any second; unpushed work is lost work.
5. Secrets never go in files, commits, issues or comments.

## Token budget rules (the factory must be cheap)
- Do NOT read AGENTS.md, FAILSAFE.md, README or whole source files. Use `python3 scripts/code_map.py [file]` (map of src) and read ranges: `sed -n 100,180p src/x.rs`.
- Read Blender with `scripts/blender_ref.sh show <path> [from] [to]` (ranges). Skim, then read only the functions you port. Never clone/print big files.
- Iterate with `cargo check -q` and `cargo test -q <filter>`. Run `factory.py check` once before `ready`. Never paste long build output; the scripts already trim it.
- No narration, no plans in prose, no summaries of what you did. Commit messages and CHECKPOINTs are one or two lines.
- Batch several shell commands into one call. Do not re-read a file you just wrote. Do not poll CI: `factory.py merge` waits inside the script for free.
- Prefer new files/modules over editing hotspots (main.rs, scene.rs, view3d.rs, scene/ops.rs); touch hotspots minimally, merge fast.
- Tests are mandatory for logic, and they are your proof. Pure functions + unit tests are cheaper and safer than UI code.

## Review (your duty before taking new work; `next` will route you)
`diff PR`, open the cited Blender files, check the Rust matches names/defaults/algorithm, no invented behaviour, tests exist and are meaningful. Then
`review PR --paths <files you opened>` or `review PR --changes "<specific problems>"`. Approve only what you verified.

## Planning (only when `next` says PLAN)
`plan-next` prints the next area and candidate Blender files. Skim the central files, write spec.json (3-8 children, size S/M, real paths, testable acceptance), `new-issues spec.json`.
