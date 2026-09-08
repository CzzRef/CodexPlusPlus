(() => {
  if (typeof window !== "object") return;
  let bridge;
  let draft = null;
  const escape = (value) => String(value ?? "").replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]);
  const style = `<style>
    .cpp-unified{padding:4px 0 16px;display:grid;gap:18px;color:var(--codex-plus-text)}
    .cpp-unified h3{font-size:18px;font-weight:600;margin:0 0 5px}.cpp-unified p{font-size:12px;line-height:1.7;opacity:.72;margin:0}
    .cpp-unified-cards{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:10px}.cpp-unified-card,.cpp-unified-section{padding:15px;border:1px solid var(--codex-plus-border,#7774);border-radius:12px;background:var(--codex-plus-surface,#7771)}
    .cpp-unified-card span{display:block;font-size:11px;opacity:.65;margin-bottom:7px}.cpp-unified-card strong{font-size:13px;font-weight:500}.cpp-unified-card [data-ready=true]{color:#379e79}
    .cpp-unified-actions{display:flex;gap:8px;flex-wrap:wrap;align-items:center}.cpp-unified button{font:inherit;font-size:12px;padding:8px 12px;border:1px solid var(--codex-plus-border,#7775);border-radius:8px;background:transparent;color:inherit;cursor:pointer}.cpp-unified button.primary{background:#276a55;border-color:#276a55;color:white}.cpp-unified button:disabled{opacity:.4;cursor:default}
    .cpp-unified label{display:grid;gap:6px;font-size:12px;min-width:0}.cpp-unified select,.cpp-unified input{box-sizing:border-box;width:100%;font:inherit;font-size:12px;border:1px solid var(--codex-plus-border,#7775);border-radius:7px;padding:9px;background:var(--codex-plus-surface,#222);color:inherit;min-width:0}
    .cpp-unified-grid{display:grid;grid-template-columns:1fr 1fr;gap:12px;margin:13px 0}.cpp-unified .wide{grid-column:1/-1}.cpp-unified [hidden]{display:none!important}.cpp-unified-section>summary{font-size:13px;cursor:pointer}.cpp-unified-note{font-size:12px;line-height:1.6;min-height:18px;overflow-wrap:anywhere}.cpp-unified-note[data-error=true]{color:#d97362}.cpp-unified-thread{font:11px ui-monospace,monospace;opacity:.6;overflow-wrap:anywhere;margin-top:8px}
    @media(max-width:720px){.cpp-unified-cards{grid-template-columns:1fr 1fr}.cpp-unified-grid{grid-template-columns:1fr}.cpp-unified .wide{grid-column:auto}}
  </style>`;

  function turnRequest(message) {
    if (message?.type === "fetch" && typeof message.url === "string" && message.url.startsWith("vscode://codex/")) {
      try {
        const body = typeof message.body === "string" ? JSON.parse(message.body) : message.body;
        return turnRequest({ ...body, type: message.url.slice("vscode://codex/".length).split(/[?#]/)[0] });
      } catch { return null; }
    }
    const request = ["mcp-request", "worker-request"].includes(message?.type) ? message.request : message;
    return request?.method === "turn/start" ? request.params : message?.type === "turn/start" ? message.params ?? message : null;
  }

  function beforeDispatch(message) {
    const params = turnRequest(message);
    if (!draft || !params || !bridge) return null;
    const threadId = params.threadId || params.thread_id || params.conversationId;
    if (!threadId) return Promise.reject(new Error("无法将暂存设置绑定到任务，请打开具体任务后再保存设置"));
    const selected = draft;
    return bridge("/unified/control", { ...selected, threadId }).then((result) => {
      if (result?.status !== "ok") throw new Error(result?.message || "任务设置未能应用");
      if (draft === selected) draft = null;
    });
  }

  function mount(container, options) {
    if (!container || container.dataset.mounted) return;
    container.dataset.mounted = "true";
    bridge = options.postJson;
    let runtime = {};
    let catalog = { models: [], profiles: [], groups: [], turns: [] };
    let busy = false;
    container.innerHTML = `${style}<div class="cpp-unified">
      <div><h3>官方、API 与 ChatGPT Web</h3><p>在 Codex 模型选择器中选择模型。每个任务独立运行，聚合供应商在一轮中保持固定。</p></div>
      <div class="cpp-unified-cards">${["统一路由", "网关", "Web 浏览器", "Web 工具连接"].map((name, index) => `<div class="cpp-unified-card"><span>${name}</span><strong data-runtime-card="${index}">读取中…</strong></div>`).join("")}</div>
      <div class="cpp-unified-actions"><button class="primary" data-action="enable">启用统一路由</button><button data-action="disable">停用并恢复</button><button data-action="login">登录 ChatGPT</button><button data-action="browser-restart">重启 Web 浏览器</button><button data-action="restart">重启组件</button><button data-action="refresh">刷新状态</button></div>
      <div class="cpp-unified-note" role="status" aria-live="polite" data-notice></div>
      <section class="cpp-unified-section"><h3>本任务的模型设置</h3><p>推理强度会在下一次模型请求生效。聚合成员的选择从下一轮生效。</p><div class="cpp-unified-thread" data-thread></div>
        <div class="cpp-unified-grid"><label class="wide">设置对应的模型<select data-model aria-label="设置对应的模型"></select></label><label>推理强度<select data-effort aria-label="推理强度"></select></label><label>聚合成员<select data-profile aria-label="聚合成员"></select></label><label class="wide">自动选择方式<select data-strategy aria-label="自动选择方式"><option value="lastUsed">默认沿用本任务上次的供应商</option><option value="strategy">按分组的轮转策略选择</option></select></label></div>
        <div class="cpp-unified-actions"><button class="primary" data-action="save-turn">保存本任务设置</button><span class="cpp-unified-note" data-binding></span></div><div class="cpp-unified-grid" data-default-group hidden><label>分组默认供应商<select data-group-default aria-label="分组默认供应商"></select></label><div class="cpp-unified-actions"><button data-action="save-group-default">保存分组默认</button></div></div>
      </section>
      <details class="cpp-unified-section" data-web-section><summary>Web 工具连接</summary><p style="margin-top:10px">Full MCP 连接让 Web 模型使用当前任务获准的文件和命令工具。填写专用 Tunnel ID 与 Runtime Key，再在 ChatGPT 中添加对应连接器。</p>
        <div class="cpp-unified-grid"><label>Web 交互方式<select data-web-mode aria-label="Web 交互方式"><option value="automatic">自动执行</option><option value="manual">手动发送（Zero Risk）</option></select></label><label>账号能力<select data-web-plan aria-label="账号能力"><option value="plus">Plus / Sol</option><option value="pro">Pro / Sol</option><option value="free">Free / Luna</option></select></label><label class="wide">Tunnel ID<input data-tunnel placeholder="tunnel_…" autocomplete="off"></label><label class="wide">Runtime Key<input data-runtime-key type="password" placeholder="留空以保留已有连接" autocomplete="new-password"></label></div>
        <button data-action="configure-web">保存 Web 连接</button><p data-connector style="margin-top:10px"></p>
      </details>
      <details class="cpp-unified-section" data-components><summary>运行组件</summary><p style="margin-top:10px">首次使用时导入组件清单，或填写下方路径。启用期间保持组件版本固定。</p><div class="cpp-unified-grid"><label class="wide">组件清单<input data-component-manifest spellcheck="false" placeholder="/absolute/path/components.json"></label></div><button data-action="import-components">导入组件清单</button><div class="cpp-unified-grid">${[["bunPath", "Bun 可执行文件"], ["runtimeEntry", "CCW cli.js"], ["electronPath", "Electron 可执行文件"], ["browserEntry", "Web 浏览器 main.cjs"]].map(([field, label]) => `<label class="wide">${label}<input data-component="${field}" spellcheck="false" autocomplete="off" placeholder="/absolute/path"></label>`).join("")}</div><button data-action="save-components">保存组件路径</button></details>
    </div>`;
    const find = (selector) => container.querySelector(selector);
    function notice(message, error = false) { const node = find("[data-notice]"); node.textContent = message; node.dataset.error = String(error); }
    function selectedRow() { return catalog.models.find(row => row.slug === find("[data-model]").value); }
    function updateControls() {
      const row = selectedRow();
      const threadId = options.threadId() || "";
      find("[data-thread]").textContent = threadId ? `任务：${threadId}` : "新任务：设置会在首次发送时绑定到该任务。";
      const pref = catalog.preferences?.[threadId] || {};
      const web = row?.slug?.startsWith("chatgpt-web/");
      const levels = Array.isArray(row?.supported_reasoning_levels) ? row.supported_reasoning_levels.map(level => typeof level === "string" ? level : level.effort) : [];
      find("[data-effort]").innerHTML = `<option value="">${web ? "由所选 Web 模式固定" : "继承 Codex 设置"}</option>${levels.map(level => `<option value="${escape(level)}">${escape(level)}</option>`).join("")}`;
      find("[data-effort]").disabled = !!web || !runtime.enabled;
      find("[data-effort]").value = pref.efforts?.[row?.slug] || "";
      const group = catalog.groups.find(group => group.id === row?.cpp_route?.groupId);
      find("[data-profile]").innerHTML = `<option value="">${group ? "自动选择" : "使用所选模型的供应商"}</option>${(group?.members || []).map(member => `<option value="${escape(member.relayId)}">${escape(catalog.profiles.find(profile => profile.id === member.relayId)?.name || member.relayId)}</option>`).join("")}`;
      find("[data-profile]").disabled = !group || !runtime.enabled;
      find("[data-profile]").closest("label").hidden = !group;
      find("[data-strategy]").closest("label").hidden = !group;
      find("[data-profile]").value = pref.groups?.[group?.id]?.profileId || "";
      find("[data-default-group]").hidden = !group;
      find("[data-group-default]").innerHTML = `<option value="">使用分组策略</option>${(group?.members || []).map(member => `<option value="${escape(member.relayId)}">${escape(catalog.profiles.find(profile => profile.id === member.relayId)?.name || member.relayId)}</option>`).join("")}`;
      find("[data-group-default]").value = group?.defaultProfileId || "";
      find('[data-action="save-group-default"]').disabled = !group || !runtime.enabled;
      find("[data-strategy]").disabled = !group || !runtime.enabled;
      find("[data-strategy]").value = pref.groups?.[group?.id]?.selectionMode || "lastUsed";
      find('[data-action="save-turn"]').disabled = !row || !runtime.enabled || !!web;
      const turn = catalog.turns.filter(turn => turn.threadId === threadId && turn.model === row?.slug).at(-1);
      find("[data-binding]").textContent = web ? "更换 Web 模式请在模型选择器中操作，下一轮生效。" : turn ? `最近一轮：${catalog.profiles.find(profile => profile.id === turn.profileId)?.name || turn.profileId}` : "";
    }
    async function refresh(initial = false) {
      const result = await bridge("/unified/runtime", { action: "status" });
      if (result?.status !== "ok") throw new Error(result?.message || "统一路由状态不可用");
      runtime = result;
      const cards = [runtime.enabled ? "已启用" : "未启用", runtime.health?.status === "ok" ? "运行中" : "未运行", runtime.browserPid ? "已启动" : "未启动", runtime.tunnelOwnerPid ? "已启动，待连接器验证" : "未配置"];
      cards.forEach((label, index) => { const node = find(`[data-runtime-card="${index}"]`); node.textContent = label; node.dataset.ready = String(index === 0 ? runtime.enabled : index === 1 ? !!runtime.health : index === 2 ? !!runtime.browserPid : !!runtime.tunnelOwnerPid); });
      find('[data-action="enable"]').disabled = !!runtime.enabled || busy;
      for (const action of ["disable", "login", "browser-restart", "restart", "configure-web"]) find(`[data-action="${action}"]`).disabled = !runtime.enabled || busy;
      if (initial) {
        container.querySelectorAll("[data-component]").forEach(input => { input.value = runtime.settings?.[input.dataset.component] || ""; });
        find("[data-components]").open = !runtime.settings?.runtimeEntry;
      }
      if (runtime.enabled) {
        const choices = await bridge("/unified/choices", {});
        if (choices?.status === "ok") {
          catalog = choices.data;
          if (initial || !container.contains(document.activeElement)) {
            const selected = find("[data-model]").value || options.currentModel?.();
            find("[data-model]").innerHTML = catalog.models.map(row => `<option value="${escape(row.slug)}">${escape(row.display_name || row.slug)}</option>`).join("");
            if (catalog.models.some(row => row.slug === selected)) find("[data-model]").value = selected;
            updateControls();
          }
        }
      } else updateControls();
      if (runtime.lastError) notice(runtime.lastError, true);
    }
    find("[data-model]").addEventListener("change", updateControls);
    container.addEventListener("click", async event => {
      const action = event.target.closest("[data-action]")?.dataset.action;
      if (!action || busy) return;
      busy = true; notice("正在处理…");
      try {
        let result;
        if (action === "save-turn") {
          const row = selectedRow();
          if (!row) throw new Error("请先选择模型");
          const preference = { model: row.slug };
          if (!row.slug.startsWith("chatgpt-web/")) preference.reasoningEffort = find("[data-effort]").value || null;
          if (row.cpp_route?.groupId) Object.assign(preference, { groupId: row.cpp_route.groupId, profileId: find("[data-profile]").value || null, selectionMode: find("[data-strategy]").value });
          const threadId = options.threadId();
          if (threadId) result = await bridge("/unified/control", { ...preference, threadId });
          else { draft = preference; result = { status: "ok" }; }
          if (result?.status !== "ok") throw new Error(result?.message || result?.error?.message || "设置未保存");
          notice(threadId ? "已保存：推理强度在下一请求生效，聚合成员在下一轮生效。" : "已暂存，将在新任务首次发送前应用。");
        } else if (action === "save-group-default") {
          result = await bridge("/unified/runtime", { action: "set-group-default", groupId: selectedRow()?.cpp_route?.groupId, profileId: find("[data-group-default]").value || null });
} else if (action === "save-components") {
          const settings = { ...runtime.settings };
          container.querySelectorAll("[data-component]").forEach(input => { settings[input.dataset.component] = input.value.trim(); });
          result = await bridge("/unified/runtime", { action, settings });
        } else if (action === "configure-web") {
          const plan = find("[data-web-plan]").value;
          const settings = { browserInteractionMode: find("[data-web-mode]").value, solAvailable: plan !== "free", proAvailable: plan === "pro", experimentalBiggerContext: false, zeroRiskProEnabled: false };
          const tunnelId = find("[data-tunnel]").value.trim(); const runtimeKey = find("[data-runtime-key]").value.trim();
          if (tunnelId || runtimeKey) Object.assign(settings, { tunnelId, runtimeKey });
          result = await bridge("/unified/runtime", { action, settings });
          find("[data-runtime-key]").value = "";
          if (result?.connectorName) find("[data-connector]").textContent = `ChatGPT 连接器名称：${result.connectorName}。完成连接器设置后，再验证真实文件和命令工具调用。`;
        } else if (action !== "refresh") result = await bridge("/unified/runtime", { action });
        if (result && !["ok", "configured"].includes(result.status)) throw new Error(result.message || "操作未完成");
        if (action !== "save-turn") notice(action === "enable" ? "统一路由已启用。重新打开 Codex 后，在模型选择器中选择新模型。" : "已完成。");
        await options.refreshSettings?.();
      } catch (error) { notice(error?.message || String(error), true); }
      finally { busy = false; await refresh().catch(error => notice(error.message, true)); }
    });
    void refresh(true).catch(error => notice(error.message, true));
    const timer = setInterval(() => { if (!container.isConnected) clearInterval(timer); else if (!container.hidden && !busy) void refresh().catch(error => notice(error.message, true)); }, 5000);
  }
  window.__codexPlusUnifiedPanel = { mount, beforeDispatch, turnRequest, setDraft: value => { draft = value; }, setBridge: value => { bridge = value; } };
})();
