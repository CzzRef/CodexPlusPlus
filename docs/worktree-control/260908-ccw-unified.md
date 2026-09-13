# Unified integration V0.1

Branch: `codex/260908-ccw-unified`; target: `czz-dev`. Paired sources are retained under the Codex++ worktree hub.

Implementation and local verification are complete: 798 affected automated tests and 9 isolated macOS component checks passed. The delivered package is RC2. Source authority: `vibe/specs/260908/1421-ccw-unified/spec.md`; current evidence: `verify.md` beside it.

Local batched submission is complete: 5 implementation commits, plus later rules-alignment and control-record merges. Child HEAD is `fe9c4e5c5b3c1fe695d047cfc6d63e9eee8077ba`. Target `czz-dev` is now `193096b1828f685baac3b764d096208b273d6a9e` after merging upstream `v1.3.0`; the child does not contain that merge. Preview of merging `czz-dev` into the child shows 11 files changed in both (launcher, manager UI, inject, core settings/runtime, launcher tests). The child worktree remains locally dirty only for nested-depth `AGENTS.md`; that relative path stays uncommitted.

The user will perform target account, native UI, Full MCP and Voice acceptance later; these checks remain not_run. There is no installed route activation, target integration or remote push. Both source worktrees are retained; lifecycle observation remains unmanaged. Next Git step is an authorized conflict resolution if the unified branch must move onto 1.3.0.
