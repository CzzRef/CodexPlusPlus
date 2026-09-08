# Verification — 2026-09-08

V0.1 implementation and isolated macOS component verification are complete. The installed Codex app, real account routing, login and tunnel have not been activated by this task.

| Check | Result | Scope |
| --- | --- | --- |
| Codex++ Rust integration tests | 371 passed | unified, protocol proxy, relay config/switch, launcher, bridge routes, model suffix |
| CCW Bun 1.4.0 tests | 150 passed | unified router/server, native passthrough, catalog, lifecycle, both compaction contracts, integration ownership |
| Electron Node tests | 116 passed | managed/DEV profiles, runtime host/supervisor, authenticated browser controls |
| Manager Node tests | 161 passed | renderer/settings contracts, native first-turn draft binding and retry |
| Compilation | passed | Rust core/launcher/Manager checks, release launcher, CCW and launcher TypeScript, Manager TypeScript/Vite, Electron renderer build |
| UI inspection | passed in offline fixture | actual injected panel at desktop and 390px width; model/effort/group controls, Web mode, connection save feedback and cleared key field |
| Actual macOS component smoke | 9 checks passed | packaged Bun and Electron, private profile, authenticated controls, API before/after browser exit, broken Web config isolation, idle shutdown, unchanged disposable Codex config |
| Package integrity | passed | four component SHA-256 values; Electron resource links remain inside the package |

The 798 automated tests are affected regressions, not the entire two-repository test inventory. Existing platform/dead-code and Vite chunk-size warnings remain. The installed global Bun stays unchanged; the declared Bun 1.4.0 was installed only under the task artifact directory.

The native TCP listener check replaced a proxy-sensitive HTTP shutdown probe. The pre-existing launcher test now follows the existing macOS retry policy. Actual component startup exposed and resolved Electron link copying, pre-tunnel Full-mode browser validation, hidden startup failure reporting and background session-refresh shutdown interference.

## Delivered artifact

The worktree hub's `artifacts/unified-v0.1-rc2/components.json` identifies the verified macOS arm64 package. Its corresponding `unified-v0.1-rc2-smoke.json` contains bounded acceptance results, not request or credential logs. Earlier package directories are superseded development intermediates.

Pinned versions: Codex++ 1.2.56 development source, CCW 5.0.5, Bun 1.4.0, Electron 41.10.7. The component manifest records the source bases and the then-uncommitted worktree content used for that build. Subsequent local commits record that implementation; the verified package has not been rebuilt or relabelled. The smoke check used a temporary profile and local API fixture, then stopped its owned processes and removed the temporary directory.

## Real account gates — not_run

- Start the built Codex++ launcher against the intended official Codex app; import the component manifest and activate the scoped route.
- Authenticate the private ChatGPT profile and configure the intended tunnel/connector.
- Run official, API and Web tasks concurrently, including each real API protocol and aggregate strategy.
- Verify native first-turn settings, mid-turn effort changes, per-turn provider choice, stream interruption/reconnect and both compaction contracts in the installed client.
- Verify actual Web file and command tools through Full MCP, the original approval boundary, real Voice, owned component restart and config recovery.

These gates require the target app/account and connector setup. No mock, build, screenshot or component boot substitutes for them. The user requested local batched commits and deferred target account tests until later. Local submission checks cover the staged scope, whitespace, document links, recorded identity and committed tree. The earlier runtime tests were not repeated for this submission. No target integration, remote push or worktree removal occurred; both worktrees remain retained under an unmanaged lifecycle observation.

CPP product guidance is in `docs/unified-routing.md`; CCW architecture and security owners document the managed profile.
