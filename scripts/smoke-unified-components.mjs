#!/usr/bin/env node
// Boot the packaged gateway and Electron helper in a disposable private profile.
// Uses only a loopback API fixture; does not activate Codex or configure a tunnel.
import assert from "node:assert/strict";
import { createHash, randomBytes } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";

const args = process.argv.slice(2);
const option = name => { const i = args.indexOf(name); return i < 0 ? undefined : args[i + 1]; };
if (!option("--components")) throw new Error("Usage: node scripts/smoke-unified-components.mjs --components /absolute/components.json [--report /absolute/summary.json]");
if (process.platform !== "darwin" || process.arch !== "arm64") throw new Error("This acceptance check requires macOS arm64");
const components = JSON.parse(readFileSync(resolve(option("--components")), "utf8"));
assert.equal(components.owner, "codex-plusplus");
for (const name of ["bunPath", "runtimeEntry", "electronPath", "browserEntry"]) {
  assert.equal(createHash("sha256").update(readFileSync(components.settings[name])).digest("hex"), components.hashes[name], `Component hash mismatch: ${name}`);
}
const root = mkdtempSync("/tmp/cpp-unified-smoke-");
const profile = join(root, "profile"); const codex = join(root, "codex");
mkdirSync(profile); mkdirSync(codex);
const original = 'model = "official-fixture"\n[features]\nvoice = true\n';
writeFileSync(join(codex, "config.toml"), original);
const token = randomBytes(32).toString("hex");
writeFileSync(join(profile, "control-token"), token, { mode: 0o600 });
const manifest = { schemaVersion: 1, ccwVersion: "5.0.5", revision: "isolated-smoke",
  profiles: [{ id: "fixture", name: "Fixture", protocol: "responses", models: ["cpp/fixture/model"] }], groups: [],
  models: [{ slug: "cpp/fixture/model", supported_reasoning_levels: [{ effort: "high" }], cpp_route: { kind: "api", profileId: "fixture", model: "model" } }] };
const checks = {}; const children = []; let descriptor; let base; let fixture;
async function listen(server) {
  await new Promise((accept, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", accept); });
  return server.address().port;
}
async function json(url, init = {}) {
  const response = await fetch(url, { ...init, signal: AbortSignal.timeout(2000) });
  assert.equal(response.status, 200, `${new URL(url).pathname} returned ${response.status}`);
  return response.json();
}
async function until(read, message, timeout = 15000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) { try { const value = await read(); if (value) return value; } catch {} await delay(100); }
  throw new Error(message);
}
function start(executable, argv) {
  const env = { ...process.env, CODEX_CPP_MANAGED: "1", CODEX_CHATGPT_WEB_HOME: profile, CODEX_HOME: codex };
  delete env.ELECTRON_RUN_AS_NODE;
  const child = spawn(executable, argv, { env, stdio: ["ignore", "pipe", "pipe"] });
  child.diagnostics = "";
  for (const stream of [child.stdout, child.stderr]) stream.on("data", data => { child.diagnostics = (child.diagnostics + data.toString()).slice(-4000); });
  child.on("error", error => { child.spawnFailure = error.code; }); children.push(child); return child;
}
async function exited(child) { return until(() => child.exitCode !== null || child.signalCode !== null, "Owned component did not exit", 10000); }
const control = action => json(`${descriptor.control.endpoint}/v1/managed/${action}`, { method: "POST", headers: { authorization: `Bearer ${descriptor.control.token}` } });
const admin = action => json(`${base}/admin/${action}`, { method: "POST", headers: { authorization: `Bearer ${token}` } });
async function apiRequest(turn) {
  return json(`${base}/v1/responses`, { method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ model: "cpp/fixture/model", input: "fixture", stream: false,
      client_metadata: { "x-codex-turn-metadata": JSON.stringify({ thread_id: "smoke-thread", turn_id: turn }) } }) });
}
try {
  fixture = createServer(async (request, response) => {
    if (request.headers.authorization !== `Bearer ${token}`) { response.writeHead(401); response.end(); return; }
    response.setHeader("content-type", "application/json");
    if (request.url === "/internal/unified/manifest") response.end(JSON.stringify(manifest));
    else if (request.url === "/internal/unified/responses") {
      let body = ""; for await (const chunk of request) body += chunk;
      assert.equal(request.headers["x-cpp-profile-id"], "fixture"); assert.equal(JSON.parse(body).model, "model");
      response.end(JSON.stringify({ status: "completed", output: [{ type: "message", content: [{ type: "output_text", text: "fixture" }] }] }));
    } else { response.writeHead(404); response.end("{}"); }
  });
  const apiPort = await listen(fixture);
  const reservation = createServer(); const gatewayPort = await listen(reservation);
  await new Promise(accept => reservation.close(accept));
  base = `http://127.0.0.1:${gatewayPort}`;
  const launchFile = join(profile, "managed-launch.json");
  writeFileSync(launchFile, JSON.stringify({ schemaVersion: 1, owner: "codex-plusplus", ccwVersion: "5.0.5", profileDir: profile,
    apiBase: `http://127.0.0.1:${apiPort}`, port: gatewayPort, subagentProtocol: "native",
    runtimeCommand: [components.settings.bunPath, components.settings.runtimeEntry] }), { mode: 0o600 });
  const gateway = start(components.settings.bunPath, [components.settings.runtimeEntry, "managed", "--config", launchFile]);
  await until(() => json(`${base}/healthz`), "Packaged gateway did not become healthy"); checks.gatewayBoot = true;
  assert.equal(JSON.parse(readFileSync(join(profile, "config.json"), "utf8")).purpose, "managed");
  assert.equal((await apiRequest("before-browser")).status, "completed"); checks.apiWithoutBrowser = true;
  const browser = start(components.settings.electronPath, [components.settings.browserEntry, "--managed-profile", "--hidden"]);
  descriptor = await until(() => { const file = join(profile, "runtime/launcher-browser.json"); return existsSync(file) && JSON.parse(readFileSync(file, "utf8")); }, "Packaged Electron helper did not publish its private descriptor")
    .catch(error => {
      const logFile = join(profile, "launcher/logs/launcher.jsonl");
      const events = existsSync(logFile) ? readFileSync(logFile, "utf8").trim().split("\n").slice(-12).map(line => {
        try { const row = JSON.parse(line); return { event: row.event, message: row.detail?.message ?? row.message }; } catch { return {}; }
      }) : [];
      const fatalFile = join(profile, "launcher/logs/launcher-fatal.log");
      const fatal = existsSync(fatalFile) ? readFileSync(fatalFile, "utf8").split("\n").slice(0, 6).join("\n") : "";
      throw new Error(`${error.message} (exit ${browser.exitCode}, spawn ${browser.spawnFailure ?? "ok"}): ${browser.diagnostics}\n${JSON.stringify(events)}\n${fatal}`);
    });
  assert.equal(descriptor.pid, browser.pid); assert.equal(descriptor.profile, "managed");
  assert.equal(descriptor.partition, "persist:codex-plus-managed-chatgpt");
  const state = await until(async () => { const value = await control("status"); return value.status === "ready" && value; }, "Packaged browser did not complete startup");
  assert.equal(state.activeTurnCount, 0); checks.browserBootAndPrivateProfile = true;
  const rejected = await fetch(`${descriptor.control.endpoint}/v1/managed/status`, { method: "POST", signal: AbortSignal.timeout(2000) });
  assert.equal(rejected.status, 401); checks.browserControlAuthentication = true;
  await control("quit"); await exited(browser).catch(error => {
    const logFile = join(profile, "launcher/logs/launcher.jsonl");
    const events = existsSync(logFile) ? readFileSync(logFile, "utf8").trim().split("\n").slice(-6).map(line => {
      try { const row = JSON.parse(line); return { event: row.event, message: row.detail?.message }; } catch { return {}; }
    }) : [];
    throw new Error(`${error.message}; browser operations: ${JSON.stringify(events)}`);
  }); checks.browserOwnedQuit = true;
  assert.equal((await apiRequest("after-browser")).status, "completed"); checks.apiAfterBrowserQuit = true;
  writeFileSync(join(profile, "config.json"), "invalid-web-config");
  assert.equal((await apiRequest("invalid-web-config")).status, "completed"); checks.apiWithBrokenWebConfig = true;
  const idle = await admin("drain"); assert.equal(idle.active_http_turns, 0); assert.equal(idle.active_browser_turns, 0);
  await admin("shutdown");
  // The managed CLI retains its main promise; after the server confirms shutdown, stop this exact owned child.
  if (gateway.exitCode === null) gateway.kill("SIGTERM");
  await exited(gateway); checks.gatewayDrainAndOwnedStop = true;
  assert.equal(readFileSync(join(codex, "config.toml"), "utf8"), original); checks.isolatedCodexConfigUnchanged = true;
  const report = { schemaVersion: 1, checkedAt: new Date().toISOString(), platform: "darwin-arm64", checks,
    notRun: ["installed Codex activation", "real official and provider requests", "ChatGPT login", "tunnel and Full MCP tools", "live Voice"] };
  if (option("--report")) writeFileSync(resolve(option("--report")), JSON.stringify(report, null, 2) + "\n");
  console.log(JSON.stringify(report, null, 2));
} finally {
  if (descriptor) await control("quit").catch(() => {});
  for (const child of children) if (child.exitCode === null && child.signalCode === null) {
    child.kill("SIGTERM"); await exited(child).catch(() => { child.kill("SIGKILL"); });
  }
  if (fixture) { fixture.closeAllConnections(); await new Promise(accept => fixture.close(accept)); }
  rmSync(root, { recursive: true, force: true });
}
