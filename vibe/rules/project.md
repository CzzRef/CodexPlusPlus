# Project Rules

Tool: tool-neutral (codex, claude, grok, and any CodeNote-routed agent)

## Project Profile

- Name: `CodexPlusPlus`（Codex++）
- Path: GitFork clone at `GitFork/CodexPlusPlus`
- Current working branch: `czz-dev`（本地 fork 主开发分支；`main` 跟踪 `origin/main`）
- Stack: Rust workspace + Tauri 2 / React+TS 管理工具
- Purpose: 面向 OpenAI Codex / ChatGPT 桌面应用的外部启动器与管理工具；不修改官方 `app.asar`，不向安装目录写补丁
- This fork goal: 按模型粒度配置上下文窗口与自动压缩阈值（issue #1171 / #931），走 codex 原生 `model_catalog_json` / `model_list` 后缀语法（如 `deepseek-v4-pro[1M]`）
- Initialization date: 2026-09-08

## Detected Manifests

- `Cargo.toml` workspace（`resolver = "2"`）
- `apps/codex-plus-manager/package.json`
- `apps/codex-plus-manager/src-tauri/`

## Code Layout

- `crates/codex-plus-core/` — 配置生成、catalog 解析、数据模型
- `crates/codex-plus-data/` — 数据持久化
- `apps/codex-plus-manager/` — Tauri 桌面管理工具，前端 React+TS
- `apps/codex-plus-launcher/` — 启动器
- `apps/codex-plus-mobile-relay/` — 移动中继
- `services/share-site/` — 分享站点
- `tools/` — 辅助工具（含 `tools/codex-wechat/`）
- `docs/` — 本 fork 的设计、调研与计划；历史产品文档仍以此为权威

Directories that do **not** exist and must not be invented: `vibe/ai-db/`, `vibe/requirements/`.

## Key Code Locations

- 数据模型：`crates/codex-plus-core/src/settings.rs` 的 `RelayProfile`
- 配置生成：`crates/codex-plus-core/src/relay_config.rs` 的 `apply_context_limits_to_config`
- catalog 解析：`crates/codex-plus-core/src/model_catalog.rs` 的 `parse_model_catalog_json_models`
- apply 入口：`crates/codex-plus-core/src/relay_config.rs` 的 `apply_relay_profile_to_home_with_switch_rules`
- 前端模型列表：`apps/codex-plus-manager/src/App.tsx` 的 `modelList` textarea

## Local Rule Policy

- Keep project-specific constraints here; move reusable cross-project rules to CodeNote.
- Preserve existing behavior and user changes; do not touch unrelated files.
- 改动隔离 + opt-in，不破坏现有 per-profile 单值行为。
- 保持上游代码风格（Rust 标准、React+TS）；不做需求外操作。
- 不擅自改 `Cargo.toml`、`package.json`、`.gitignore`（除非当前任务必需）。
- 覆盖文件前确认；删除只能单个文件，删除前确认。禁止批量删除、`rm -rf`、`rmdir /s`。
- 禁止 sudo、提权、`curl | bash`。
- 对话用中文，代码可用英文，注释尽量中文。

## Upstream Sync

- `origin` = `git@github.com:CzzRef/CodexPlusPlus.git`
- `upstream`（约定，本机尚未强制配置）= `https://github.com/BigPizzaV3/CodexPlusPlus.git`
- 功能分支可用 `codex/per-model-context` 或类似；日常开发落在 `czz-dev`
- 定期 `git fetch upstream && git rebase upstream/main` 保持同步
- 目标：全栈完成后向主仓提 PR

## High-Risk Areas

- 密钥与本机凭据：`.env`、`auth.json`、`~/.codex/config.toml`、`tools/codex-wechat/config.local.json`
- 覆盖或改写用户本机 Codex home 配置
- 发布/安装包、对官方桌面应用安装目录的任何写入
- 应用内 rusqlite 本地库是产品数据，不是 AI-DB 工作区；不要为此创建 `vibe/ai-db/`
