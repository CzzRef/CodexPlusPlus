# Unified integration V0.1

Branch: `codex/260908-ccw-unified`; target: `czz-dev`. Paired sources are retained under the Codex++ worktree hub.

Implementation and local verification are complete: 798 affected automated tests and 9 isolated macOS component checks passed. The delivered package is RC2. Source authority: `vibe/specs/260908/1421-ccw-unified/spec.md`; current evidence: `verify.md` beside it.

The paired children now contain the later `czz-dev` heads: Codex++ child `00548e1` includes upstream `v1.3.0`; CCW child `bc26ff0` includes `5.0.6`. Target integration and remote push remain not started. Each child worktree is locally dirty only for nested-depth `AGENTS.md`.

Focused child checks: Codex++ `launcher` 85 and `unified` 13 passed; CCW managed/unified/cli 41 passed. The prior RC2 package was not rebuilt. Account, Full MCP and Voice acceptance remain not_run. Lifecycle observation remains unmanaged. Next Git step is optional component rebuild if a live GPT package must pick up 5.0.6, or push if origin should follow.
