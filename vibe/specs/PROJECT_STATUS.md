# CodexPlusPlus Project Status

Tool: cursor
Date: 2026-09-13

## Current Main Line

- AI 规则初始化（2026-09-08，主检出 `czz-dev`）：官方 `project-rules` 首次投影已 apply；入口只路由到 CodeNote。任务卡见 [ai-rules-init](260908/1204-ai-rules-init/task-card.md)。
- `czz-dev` 已同步上游 `v1.3.0`（`193096b`，本地超前 origin 65，未推送）。
- Unified V0.1 仍在 `codex/260908-ccw-unified`，未合入 1.3.0；与上游有 11 个代码文件双方都改。主目录观察见 [260908-ccw-unified](../../docs/worktree-control/260908-ccw-unified.md)。真实账号验收仍未跑。

## AI Rule Routing

- Global master: [../../../../CzzProj/CodeNote/AiRef/VibePractice/Vibe_Rules/VibeAi.md](../../../../CzzProj/CodeNote/AiRef/VibePractice/Vibe_Rules/VibeAi.md)
- Project documentation rules: [../rules/documentation.md](../rules/documentation.md)
- Specs index: [README.md](README.md)

## Active Task Index

| Task | Status | Authoritative Doc | Verification | Notes |
| --- | --- | --- | --- | --- |
| AI rules init | `implemented-local` | [task-card](260908/1204-ai-rules-init/task-card.md) | `audit_ai_rules.py --mode project` OK | CodeNote catalog 另仓未提交 |
| Unified V0.1 | `local-committed / unpushed / not-integrated` | [control record](../../docs/worktree-control/260908-ccw-unified.md) | 798 tests + 9 macOS checks passed; account/Full MCP/Voice `not_run` | Child 未含 1.3.0；合入预览 11 文件冲突 |
| Sync upstream 1.3.0 | `merged-local / unpushed` | `czz-dev` @ `193096b` | `codex-plus-core` 369 passed / 4 failed (settings default `tools` vs 空 map；vision connection_refused vs timeout)；`settings.rs`/`vision.rs` 与 `upstream/main` 无 diff | 未推 origin；未合入 unified 工作分支 |

## Governance Baseline

- Template propagation: accepted global baseline; this project keeps only project-specific routes and does not copy mother-board rules.
- Codex evolution: `v3-route-accepted`; no Hook, supervisor, or rollout change in this repository.
- Rule Task Trace: accepted global baseline; this initialization is project-local and does not add a CodeNote registry row.
- `w24-primary-objective-continuity-accepted`: primary user work remains ahead of advisory governance lanes.
- `w28-documentation-impact-accepted`: this round synchronized project-current adapters/hubs and CodeNote catalog.
- `w30-standard-requirement-owner-accepted`: Standard requirement ownership remains raw requirement plus the Spec owner. This initialization is Standard non-requirement.

## Memory Routing

- Project rules: created this round from former root `AGENTS.md`
- Knowledge: index + empty error-memory/ADR created this round
- DB memory: not configured; initialize through CodeNote DB governance only when DB/data work becomes active.

## Cross-Repository Links

| Concern | Repository | Status Hub |
| --- | --- | --- |
| Rule kernel / catalog | CodeNote | [project-index.json](../../../../CzzProj/CodeNote/vibe/knowledge/project-index.json) |
| ChatGPT Web gateway (CCW) | codex-chatgpt-web | paired hub `CodexPlusPlus-worktrees/codex/260908-ccw-unified` |

## Next Update Trigger

Update this hub when current focus, active task docs, verification status, open gates, sibling links, or memory routing changes.
