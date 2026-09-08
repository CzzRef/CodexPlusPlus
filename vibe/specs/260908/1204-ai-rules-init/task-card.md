# Task Card

> Standard non-requirement work only.

Tool: cursor
Date: 2026-09-08
Task: 1204-ai-rules-init

## Goal And Scope

- Goal: 按 CodeNote AI 规则初始化本仓库的项目适配器链，使 Cursor / Codex / Claude 能自动发现项目入口。
- In scope: 短 `AGENTS.md`；`.cursor/rules/project.mdc`；`CLAUDE.md`；`vibe/rules`；`vibe/knowledge`；`vibe/specs`；CodeNote `project-index` + 本机 `workspace.local.json` 绑定；官方 `project-rules` 首次投影。
- Out of scope: 应用代码；提交/推送；启用 intent-note / design-preference / AI-DB；迁移 `docs/` 既有方案包。
- Success evidence: 官方投影落地；project audit 对 P0 为绿；原根 `AGENTS.md` 项目事实保留在 `vibe/rules/local-context.md` / `project.md`。

## Decision

- Documentation level: `standard`
- Execution: `main-only`
- Documentation impact: `project-current`
- High-risk / DB boundary: none；不创建 `vibe/ai-db/`。

## Prior Task Overlap

- Relationship: `reference-only`
- Prior authority: GitFork `codex-host` 的 `260901/2034-ai-rules-init` 与 CodeNote starter-kit / adapter-template-pack。
- Decision: `new-task`；不重跑那些迁移，只套同一形状。

## Verification

- Commands: `configure_agent_ecosystem.py apply --components project-rules --projects codex-plusplus` → `applied` 16 files, manifest `20260908T040945528298Z.json`；`audit_ai_rules.py --mode project` → `OK`
- Status: local implementation complete; not committed

## Closeout

- Process document status: created this task card and hub
- Memory routing: project rules + empty knowledge indexes
