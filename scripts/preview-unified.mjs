#!/usr/bin/env node
import { createServer } from "node:http";
import { readFileSync } from "node:fs";
const files = {
  "/": [new URL("../docs/previews/unified-routing.html", import.meta.url), "text/html; charset=utf-8"],
  "/assets/inject/unified-panel.js": [new URL("../assets/inject/unified-panel.js", import.meta.url), "text/javascript; charset=utf-8"],
};
const server = createServer((request, response) => {
  const entry = files[new URL(request.url, "http://localhost").pathname];
  if (!entry) { response.writeHead(404); response.end(); return; }
  response.writeHead(200, { "content-type": entry[1], "cache-control": "no-store" }); response.end(readFileSync(entry[0]));
});
server.listen(0, "127.0.0.1", () => console.log(`http://127.0.0.1:${server.address().port}`));
process.on("SIGTERM", () => server.close());
process.on("SIGINT", () => server.close());
