# Decisions log
Format: date - decision - Blender source reference - author.

- Project license: GPL-3.0-or-later (Blender is GPL-2.0-or-later and we port its code).
- Single binary crate for now; new features go in new modules to avoid conflicts in hotspot files.
- Reference repo is read-only and lives outside this repo.

- 2026-11-26: Work is routed by `scripts/factory.py next` (priority: fix main, fix own PRs, merge, review, takeover, claim, plan). Agents read `docs/QUICK.md` only; the Blender reference is a partial clone read on demand. Reason: many parallel agents, minimal token use.
