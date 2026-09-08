# Workflow Rules

Tool: tool-neutral (codex, claude, grok, and any CodeNote-routed agent)

## Commands

Prefer documented project scripts over inventing new ones. Do not run unknown scripts or install new toolchains without confirmation.

```bash
cargo test -p codex-plus-core
cargo test -p codex-plus-data
```

- 测试沿用上游 `#[test]` + tempfile 风格（见 `crates/codex-plus-core/tests/relay_config.rs`）。
- 断言可读 `config.toml` 文本，如 `assert!(config.contains("model_catalog_json"))`。
- 改行为要同步改/加对应测试。
- 前端改动落在 `apps/codex-plus-manager/`；不要另起前端工具链。
- 执行 bash 前确认；不擅自装依赖。

## Verification

- 小范围规则/文档改动跑下面的 CodeNote project audit，不要为了证明文档改动去编译整个 workspace。
- 代码改动跑最近的 `cargo test` 包，或给出最小手工验证路径。
- 未执行的检查不得标为通过；记录跳过原因。
- 文档改动校验 Markdown 链接并记录未解析链接。

## Required AI Rule Audit

From the repository root:

```bash
python3 ../../CzzProj/CodeNote/AiRef/VibePractice/Vibe_Rules/scripts/audit_ai_rules.py . --mode project --fix-links
python3 ../../CzzProj/CodeNote/AiRef/VibePractice/Vibe_Rules/scripts/audit_ai_rules.py . --mode project
```
