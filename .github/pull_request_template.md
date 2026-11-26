## What and why
Closes #   <!-- REQUIRED by CI: 'Closes #N', 'Part of #N' (for epics' children use Closes) or 'Resumes #N' -->
Resumes #   <!-- only when taking over a stale PR; delete otherwise -->

## Blender source ported from
<!-- REQUIRED, checked by CI. One full repo-relative path per line, from github.com/aymenguentri1/blender, e.g.: -->
- `source/blender/editors/mesh/editmesh_select.cc`

## Checklist
- [ ] Claimed the issue (CLAIM comment + `status:claimed`), no other open PR covers this
- [ ] Size S or M, pushed in small steps, CHECKPOINT comments posted (docs/FAILSAFE.md)
- [ ] Rebased on latest `main`
- [ ] `cargo build --all-targets` and `cargo test` pass
- [ ] No unrelated changes, no new dependency, no secrets
- [ ] README / `docs/DECISIONS.md` updated for user-visible features or deviations
