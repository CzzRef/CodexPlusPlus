# Codex++ / ChatGPT Web unified V0.1

Status: V0.1 implementation and local verification complete in paired worktrees; delivered through local batched commits. The user will perform target account acceptance later.

## Requirements and acceptance

- Codex++ owns configuration and process lifecycle; official, API and Web models share the official Codex selector and support concurrent independent tasks.
- Official model IDs remain unchanged; API IDs are `cpp/<profile-id>/<model>`, aggregates `cpp-agg/<group-id>/<model>`; Web retains `chatgpt-web/*`.
- Preserve official authentication, Voice, native multiagent configuration, request identity, streaming and cancellation. API credentials never enter the public model manifest or official passthrough.
- Include Responses, Chat Completions, all existing API modes and aggregate strategies in V0.1. Explicit profile selection never changes global active profile.
- Aggregate selection is fixed per turn: manual choice, thread last-used choice, group default, then configured strategy. Request/weighted rotation advances per turn; independent groups retain independent state. No fallback after content or tools have been delivered.
- Reasoning effort means model effort, not financial quota. Supported effort changes apply to the next HTTP model request; in-flight request is untouched. Web mode switches apply at the next turn.
- API compaction supports both `/responses/compact` and a `compaction_trigger` through a tools-disabled summarization request with the bound API provider. Decode the bridge-readable envelope before API conversion; unknown opaque history fails explicitly.
- Legacy routing is the default. Unified activation uses scoped atomic config changes with a recovery journal; disable restores owned keys while preserving unrelated changes and rejects conflicting edits.
- Managed CCW writes only its caller-owned profile. Existing setup/update/doctor/uninstall and startup paths cannot rewrite official config/auth/cache in managed mode. Gateway, browser and tunnel are supervised independently; browser failure does not stop official/API routing.
- macOS first. Retain Full MCP for Web files/commands, a distinct private login partition and the existing connector flow. Cross-backend history migration and cross-backend subagent delegation are out of scope.
- Offline correctness, compilation and mock protocol tests are separate from real macOS login/tool/stream/Voice acceptance. No real user route activation is implied by implementation tests.

## Architecture

Official Codex built-in openai -> managed Bun gateway 127.0.0.1:17841 -> official backend, CCW Web bridge, or Codex++ explicit API executor 127.0.0.1:57321. Codex++ is the sole settings/catalog writer. Managed runtime uses pinned component versions, absolute executable paths and a private profile under the Codex++ data directory. Secrets stay in private settings/control files.

## Verification decision

Behavior changes require focused protocol, ownership, concurrency and failure tests plus affected Rust/TypeScript checks. Real environment acceptance remains open until actually observed. Keep both source histories and original checkouts recoverable; no remote push or target integration is authorized.
