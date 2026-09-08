import assert from "node:assert/strict";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";

function panel() {
  const window: Record<string, any> = {};
  runInNewContext(readFileSync(new URL("../../../assets/inject/unified-panel.js", import.meta.url), "utf8"), { window });
  return window.__codexPlusUnifiedPanel;
}

test("draft settings bind to the canonical first-turn thread for every supported native envelope", async () => {
  for (const message of [
    { type: "send-cli-request-for-host", method: "turn/start", params: { threadId: "new-thread" } },
    { type: "worker-request", request: { method: "turn/start", params: { threadId: "new-thread" } } },
    { type: "mcp-request", request: { method: "turn/start", params: { thread_id: "new-thread" } } },
    { type: "fetch", url: "vscode://codex/send-cli-request-for-host", body: JSON.stringify({ method: "turn/start", params: { threadId: "new-thread" } }) },
  ]) {
    const api = panel(); const calls: any[] = [];
    api.setBridge(async (path: string, body: unknown) => { calls.push({ path, body }); return { status: "ok" }; });
    api.setDraft({ model: "cpp-agg/group/model", groupId: "group", profileId: "provider" });
    await api.beforeDispatch(message);
    assert.equal(calls.length, 1); assert.equal(calls[0].path, "/unified/control");
    assert.equal(calls[0].body.threadId, "new-thread"); assert.equal(calls[0].body.profileId, "provider");
    assert.equal(api.beforeDispatch(message), null);
  }
});

test("draft preference failure stays recoverable and unrelated messages cannot claim the draft", async () => {
  const api = panel(); api.setDraft({ model: "cpp/a/model", reasoningEffort: "high" });
  let attempts = 0;
  api.setBridge(async () => ++attempts === 1 ? { status: "failed", message: "temporary failure" } : { status: "ok" });
  assert.equal(api.beforeDispatch({ type: "read-thread" }), null);
  const message = { type: "send-cli-request-for-host", method: "turn/start", params: { threadId: "new-thread" } };
  await assert.rejects(api.beforeDispatch(message), /temporary failure/);
  await api.beforeDispatch(message); assert.equal(attempts, 2);
});
