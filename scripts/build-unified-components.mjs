#!/usr/bin/env node
// Assemble the paired worktree into a local macOS integration artifact. Does not install or start it.
import { cpSync, existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const args = process.argv.slice(2);
const value = flag => { const index = args.indexOf(flag); if (index < 0 || !args[index + 1]) throw new Error(`Missing ${flag}`); return resolve(args[index + 1]); };
const ccw = value("--ccw-root"); const bun = value("--bun"); const output = value("--output"); const launcher = value("--launcher");
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
if (process.platform !== "darwin" || process.arch !== "arm64") throw new Error("Unified V0.1 packaging targets macOS arm64");
if (existsSync(output)) throw new Error("Choose a new output directory; existing artifacts are retained");
const pkg = JSON.parse(readFileSync(join(ccw, "package.json"), "utf8"));
const browserPkg = JSON.parse(readFileSync(join(ccw, "launcher/package.json"), "utf8"));
if (pkg.version !== "5.0.6" || pkg.packageManager !== "bun@1.4.0" || browserPkg.devDependencies.electron !== "41.10.7") throw new Error("Paired component version pins differ from V0.1");
function run(command, argv, cwd) {
  const result = spawnSync(command, argv, { cwd, stdio: "inherit", env: { ...process.env, ELECTRON_RUN_AS_NODE: undefined } });
  if (result.status !== 0) throw new Error(`Component build failed (${result.status ?? result.error?.code})`);
}
const bunVersion = spawnSync(bun, ["--version"], { encoding: "utf8" });
if (bunVersion.status !== 0 || bunVersion.stdout.trim() !== "1.4.0") throw new Error("Bun 1.4.0 is required");
if (!isAbsolute(launcher) || !statSync(launcher).isFile()) throw new Error("Build the Codex++ launcher first");
const require = createRequire(join(ccw, "launcher/package.json"));
const electron = require("electron");
const electronApp = resolve(dirname(electron), "../..");
if (!electronApp.endsWith("Electron.app") || !existsSync(electron)) throw new Error("Install the pinned macOS Electron distribution first");
mkdirSync(output, { recursive: true });
run(bun, ["run", "build:renderer"], join(ccw, "launcher"));
run(bun, [join(ccw, "scripts/build-runtime-bundle.ts"), join(output, "gateway")], ccw);
mkdirSync(join(output, "browser"));
for (const item of ["electron", "dist", "assets", "package.json"]) cpSync(join(ccw, "launcher", item), join(output, "browser", item), { recursive: true, verbatimSymlinks: true });
// Framework resources resolve through bundle-relative links; rebasing them to the source breaks macOS helper processes.
cpSync(electronApp, join(output, "electron/Electron.app"), { recursive: true, verbatimSymlinks: true });
mkdirSync(join(output, "bin")); cpSync(launcher, join(output, "bin/codex-plus-plus"));
cpSync(join(root, "LICENSE"), join(output, "LICENSE-CodexPlusPlus"));
const settings = {
  bunPath: join(output, "gateway/runtime/bun"), runtimeEntry: join(output, "gateway/app/cli.js"),
  electronPath: join(output, "electron/Electron.app/Contents/MacOS/Electron"), browserEntry: join(output, "browser/electron/main.cjs"), groupDefaults: {},
};
const sha256 = file => createHash("sha256").update(readFileSync(file)).digest("hex");
const sources = {};
for (const [name, directory] of [["codex-plusplus", root], ["codex-chatgpt-web", ccw]]) {
  const git = spawnSync("git", ["rev-parse", "HEAD"], { cwd: directory, encoding: "utf8" });
  sources[name] = { baseCommit: git.stdout.trim(), workingTreeIncluded: true };
}
const manifest = { schemaVersion: 1, owner: "codex-plusplus", ccwVersion: pkg.version, bunVersion: "1.4.0", electronVersion: "41.10.7", platform: "darwin-arm64", settings, sources,
  hashes: Object.fromEntries(Object.entries(settings).filter(([, file]) => typeof file === "string").map(([key, file]) => [key, sha256(file)])),
};
writeFileSync(join(output, "components.json"), JSON.stringify(manifest, null, 2) + "\n");
writeFileSync(join(output, "README.txt"), "Codex++ unified routing V0.1 local artifact\n\nThis package has not been installed or activated. Open the unified-model panel in the built Codex++ launcher and import components.json. All four component paths are local to this package.\nThe gateway includes its dependency/license manifest. The browser uses a private login profile when explicitly enabled.\nReal official/API/Web requests and Full MCP need acceptance with the intended account and tunnel.\n");
console.log(`Built ${join(output, "components.json")}`);
