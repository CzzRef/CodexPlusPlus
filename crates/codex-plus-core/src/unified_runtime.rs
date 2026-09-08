//! The launcher owns the gateway, browser and tunnel independently. No process-name kills.
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::{Context, bail};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::settings::{BackendSettings, SettingsStore};
use crate::unified::{self, CCW_VERSION, GATEWAY_PORT, RoutingMode};

#[derive(Clone)]
struct Component {
    executable: PathBuf,
    entry: PathBuf,
    entry_hash: String,
    executable_hash: String,
}

impl Component {
    fn checked(executable: &str, entry: &str) -> anyhow::Result<Self> {
        let executable = PathBuf::from(executable);
        let entry = PathBuf::from(entry);
        if !executable.is_absolute() || !executable.is_file() || !entry.is_absolute() || !entry.is_file() { bail!("Component requires existing absolute executable and entry paths"); }
        let entry_hash = format!("{:x}", Sha256::digest(std::fs::read(&entry)?));
        let executable_hash = format!("{:x}", Sha256::digest(std::fs::read(&executable)?));
        Ok(Self { executable, entry, entry_hash, executable_hash })
    }

    fn command(&self, profile: &Path) -> anyhow::Result<Command> {
        if format!("{:x}", Sha256::digest(std::fs::read(&self.executable)?)) != self.executable_hash || format!("{:x}", Sha256::digest(std::fs::read(&self.entry)?)) != self.entry_hash { bail!("Pinned component changed; restart the integration explicitly"); }
        let mut command = Command::new(&self.executable);
        command.arg(&self.entry).env("CODEX_CPP_MANAGED", "1")
            .env("CODEX_CHATGPT_WEB_HOME", profile)
            .env("CODEX_WEB_GPT_LAUNCHER_DATA_DIR", profile.join("launcher"))
            .env_remove("ELECTRON_RUN_AS_NODE")
            .stdout(Stdio::null()).stderr(Stdio::null()).stdin(Stdio::null())
            .kill_on_drop(true);
        Ok(command)
    }
}

#[derive(Default)]
struct Runtime {
    enabled: bool,
    monitoring: bool,
    gateway_spec: Option<Component>,
    browser_spec: Option<Component>,
    gateway: Option<Child>,
    browser: Option<Child>,
    tunnel: Option<Child>,
    gateway_restarts: u8,
    browser_restarts: u8,
    tunnel_restarts: u8,
    last_error: Option<String>,
}

static RUNTIME: OnceLock<Arc<Mutex<Runtime>>> = OnceLock::new();
fn runtime() -> Arc<Mutex<Runtime>> { RUNTIME.get_or_init(|| Arc::new(Mutex::new(Runtime::default()))).clone() }
fn operations() -> &'static Mutex<()> {
    static OPERATIONS: OnceLock<Mutex<()>> = OnceLock::new();
    OPERATIONS.get_or_init(|| Mutex::new(()))
}

fn token() -> anyhow::Result<String> {
    let file = unified::profile_dir().join("control-token");
    if file.is_symlink() { bail!("Managed credential must not be a symlink"); }
    let value = std::fs::read_to_string(file)?;
    if value.trim().len() < 32 { bail!("Managed control credential is invalid"); }
    Ok(value.trim().into())
}

fn local_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(5)).build()?)
}

pub async fn gateway_control(endpoint: &str, body: Option<Value>) -> anyhow::Result<Value> {
    let client = local_client()?;
    let url = format!("http://127.0.0.1:{GATEWAY_PORT}{endpoint}");
    let request = if let Some(body) = body { client.post(url).json(&body) } else { client.get(url) };
    let response = request.bearer_auth(token()?).send().await?;
    let status = response.status();
    let value: Value = response.json().await?;
    if !status.is_success() { bail!("Managed gateway operation failed ({status}): {}", value["error"]["message"].as_str().unwrap_or("unavailable")); }
    Ok(value)
}

async fn health() -> anyhow::Result<Value> {
    Ok(local_client()?.get(format!("http://127.0.0.1:{GATEWAY_PORT}/healthz")).send().await?.error_for_status()?.json().await?)
}

fn start_gateway(spec: &Component) -> anyhow::Result<Child> {
    let profile = unified::profile_dir();
    Ok(spec.command(&profile)?.arg("managed").arg("--config").arg(profile.join("managed-launch.json")).spawn()?)
}

fn start_browser(spec: &Component) -> anyhow::Result<Child> {
    Ok(spec.command(&unified::profile_dir())?.args(["--managed-profile", "--hidden"]).spawn()?)
}

fn start_tunnel(spec: &Component) -> anyhow::Result<Option<Child>> {
    let profile = unified::profile_dir();
    let web_file = profile.join("config.json");
    let web: Value = if web_file.exists() { serde_json::from_slice(&std::fs::read(web_file)?)? } else { json!({}) };
    if !web.get("tunnel").is_some_and(Value::is_object) { return Ok(None); }
    Ok(Some(spec.command(&profile)?.args(["managed", "tunnel", "--config"]).arg(profile.join("managed-launch.json")).stdin(Stdio::piped()).spawn()?))
}

pub async fn ensure_started(settings: &BackendSettings, helper_port: u16) -> anyhow::Result<()> {
    let _operation = operations().lock().await;
    ensure_started_inner(settings, helper_port).await
}

async fn ensure_started_inner(settings: &BackendSettings, helper_port: u16) -> anyhow::Result<()> {
    if settings.routing_mode != RoutingMode::Unified { return Ok(()); }
    if !cfg!(target_os = "macos") { bail!("Unified V0.1 currently supports macOS"); }
    unified::public_manifest(settings)?;
    let state = runtime();
    let mut state = state.lock().await;
    if state.enabled {
        let current = health().await?;
        if current["pid"].as_u64() != state.gateway.as_ref().and_then(Child::id).map(u64::from) { bail!("Gateway identity changed"); }
        return Ok(());
    }
    if tokio::net::TcpStream::connect(("127.0.0.1", GATEWAY_PORT)).await.is_ok() { bail!("Gateway port 17841 is occupied; refusing to take over another runtime"); }
    let gateway = Component::checked(&settings.unified.bun_path, &settings.unified.runtime_entry)?;
    // The release version is pinned independently of filesystem paths.
    let package = [gateway.entry.parent().unwrap().join("package.json"), gateway.entry.parent().unwrap().parent().unwrap().join("package.json")].into_iter().find(|p| p.is_file()).context("CCW package manifest is missing")?;
    let package: Value = serde_json::from_slice(&std::fs::read(package)?)?;
    if package["version"] != CCW_VERSION || package["name"] != "codex-chatgpt-web" { bail!("CCW component version mismatch"); }
    let bun_version = Command::new(&gateway.executable).arg("--version").output().await?;
    let expected_bun = package["packageManager"].as_str().and_then(|value| value.strip_prefix("bun@")).context("CCW Bun version pin is missing")?;
    if !bun_version.status.success() || String::from_utf8_lossy(&bun_version.stdout).trim() != expected_bun { bail!("Bun version does not match the CCW component pin ({expected_bun})"); }
    let profile = unified::profile_dir();
    std::fs::create_dir_all(&profile)?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&profile, std::fs::Permissions::from_mode(0o700))?; }
    let token_file = profile.join("control-token");
    if !token_file.exists() { unified::write_private(&token_file, format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple()).as_bytes())?; }
    token()?;
    let codex_home = crate::relay_config::default_codex_home_dir();
    let codex_config = std::fs::read_to_string(codex_home.join("config.toml")).unwrap_or_default();
    let codex_doc = codex_config.parse::<toml_edit::DocumentMut>()?;
    let protocol = if codex_doc.get("features").and_then(|v| v.get("multi_agent_v2")).and_then(toml_edit::Item::as_bool) == Some(true) { "native" } else { "compatibility-v1" };
    let launch = json!({"schemaVersion":1,"owner":"codex-plusplus","ccwVersion":CCW_VERSION,"profileDir":profile,"apiBase":format!("http://127.0.0.1:{helper_port}"),"port":GATEWAY_PORT,"subagentProtocol":protocol,"nativeContext":unified::native_context_override(&codex_home)?,"runtimeCommand":[gateway.executable,gateway.entry]});
    unified::write_private(&profile.join("managed-launch.json"), &serde_json::to_vec_pretty(&launch)?)?;
    state.gateway = Some(start_gateway(&gateway)?);
    let expected_pid = state.gateway.as_ref().and_then(Child::id).context("Gateway PID missing")?;
    let mut ready = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while tokio::time::Instant::now() < deadline {
        if state.gateway.as_mut().unwrap().try_wait()?.is_some() { break; }
        if let Ok(current) = health().await {
            if current["pid"] == expected_pid && current["version"] == CCW_VERSION && current["purpose"] == "managed" { ready = true; break; }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if !ready { if let Some(mut child) = state.gateway.take() { let _ = child.kill().await; } bail!("Managed gateway did not become ready"); }
    let hook_args = [gateway.executable.to_string_lossy().to_string(), gateway.entry.to_string_lossy().to_string(), "managed".into(), "interrupt".into(), "--config".into(), profile.join("managed-launch.json").to_string_lossy().to_string()];
    let hook_command = hook_args.iter().map(|arg| format!("'{}'", arg.replace('\'', "'\"'\"'"))).collect::<Vec<_>>().join(" ");
    let known_catalogs = settings.relay_profiles.iter().map(|profile| codex_home.join("model-catalogs").join(format!("{}.json", crate::relay_config::sanitize_catalog_filename(&profile.id)))).collect::<Vec<_>>();
    if let Err(error) = unified::set_route_with_legacy_catalogs(&codex_home, true, GATEWAY_PORT, Some(&hook_command), &known_catalogs) {
        if let Some(mut child) = state.gateway.take() { let _ = child.kill().await; }
        return Err(error);
    }
    state.last_error = None;
    state.gateway_spec = Some(gateway.clone());
    state.enabled = true;
    state.gateway_restarts = 0; state.browser_restarts = 0; state.tunnel_restarts = 0;
    match Component::checked(&settings.unified.electron_path, &settings.unified.browser_entry) {
        Ok(spec) => {
            match start_browser(&spec) { Ok(child) => state.browser = Some(child), Err(error) => state.last_error = Some(format!("Web browser failed to start: {error}")) }
            state.browser_spec = Some(spec);
        }
        Err(_) => state.last_error = Some("Web browser component is not configured; official and API routes are ready".into()),
    }
    match start_tunnel(&gateway) { Ok(child) => state.tunnel = child, Err(error) => state.last_error = Some(error.to_string()) }
    if !state.monitoring {
        state.monitoring = true;
        tokio::spawn(async {
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let shared = runtime();
                let mut state = shared.lock().await;
                if !state.enabled { state.monitoring = false; break; }
                if state.gateway.as_mut().is_some_and(|child| child.try_wait().ok().flatten().is_some()) {
                    state.gateway = None;
                    if state.gateway_restarts < 3 {
                        state.gateway_restarts += 1;
                        if let Some(spec) = &state.gateway_spec { match start_gateway(spec) { Ok(child) => state.gateway = Some(child), Err(error) => state.last_error = Some(error.to_string()) } }
                    } else { state.last_error = Some("Gateway repeatedly exited; explicit restart is required".into()); }
                }
                if state.browser.as_mut().is_some_and(|child| child.try_wait().ok().flatten().is_some()) {
                    state.browser = None;
                    if state.browser_restarts < 3 {
                        state.browser_restarts += 1;
                        if let Some(spec) = &state.browser_spec { match start_browser(spec) { Ok(child) => state.browser = Some(child), Err(error) => state.last_error = Some(error.to_string()) } }
                    } else { state.last_error = Some("Web browser repeatedly exited; official/API routing remains available".into()); }
                }
                if state.tunnel.as_mut().is_some_and(|child| child.try_wait().ok().flatten().is_some()) {
                    state.tunnel = None;
                    if state.tunnel_restarts < 3 {
                        state.tunnel_restarts += 1;
                        if let Some(spec) = &state.gateway_spec { match start_tunnel(spec) { Ok(child) => state.tunnel = child, Err(error) => state.last_error = Some(error.to_string()) } }
                    } else { state.last_error = Some("Web tunnel repeatedly exited; official/API routing remains available".into()); }
                }
            }
        });
    }
    Ok(())
}

pub async fn status() -> Value {
    let shared = runtime();
    let state = shared.lock().await;
    let settings = SettingsStore::default().load().unwrap_or_default();
    json!({"status":"ok","routingMode":settings.routing_mode,"settings":settings.unified,"enabled":state.enabled,"gatewayPid":state.gateway.as_ref().and_then(Child::id),"browserPid":state.browser.as_ref().and_then(Child::id),"tunnelOwnerPid":state.tunnel.as_ref().and_then(Child::id),"lastError":state.last_error,"health":health().await.ok()})
}

/// Drain both HTTP and browser work before stopping any child. A busy runtime remains owned.
pub async fn stop(restore_route: bool) -> anyhow::Result<()> {
    let _operation = operations().lock().await;
    stop_inner(restore_route).await
}

async fn stop_tunnel_child(child: &mut Child) -> anyhow::Result<()> {
    if let Some(mut stdin) = child.stdin.take() { stdin.write_all(b"stop\n").await?; }
    let result = tokio::time::timeout(Duration::from_secs(35), child.wait()).await.context("Owned Web tunnel is still stopping")??;
    if !result.success() { bail!("Web tunnel owner could not confirm its shutdown"); }
    Ok(())
}

async fn stop_inner(restore_route: bool) -> anyhow::Result<()> {
    let shared = runtime();
    let mut state = shared.lock().await;
    if !state.enabled {
        if restore_route { unified::set_route(&crate::relay_config::default_codex_home_dir(), false, GATEWAY_PORT)?; }
        return Ok(());
    }
    let gateway_alive = match state.gateway.as_mut() { Some(child) => child.try_wait()?.is_none(), None => false };
    let current = if gateway_alive { gateway_control("/admin/drain", Some(json!({}))).await? } else { json!({"active_http_turns":0,"active_browser_turns":0}) };
    let browser_alive = match state.browser.as_mut() { Some(child) => child.try_wait()?.is_none(), None => false };
    let browser_active = if browser_alive {
        match browser_control_for_pid("status", state.browser.as_ref().and_then(Child::id)).await {
            Ok(value) => {
                if value["activeOperation"].as_str().is_some_and(|operation| operation != "session refresh") {
                    let _ = gateway_control("/admin/resume", Some(json!({}))).await;
                    bail!("Finish the active Web login or browser operation before stopping the integration");
                }
                value["activeTurnCount"].as_u64().unwrap_or(1)
            },
            Err(error) => { let _ = gateway_control("/admin/resume", Some(json!({}))).await; return Err(error); }
        }
    } else { 0 };
    if current["active_http_turns"].as_u64().unwrap_or(1) > 0 || current["active_browser_turns"].as_u64().unwrap_or(1) > 0 || browser_active > 0 || unified::active_request_count()? > 0 {
        let _ = gateway_control("/admin/resume", Some(json!({}))).await;
        bail!("Active HTTP or Web turns remain; finish or cancel them before stopping the integration");
    }
    if let Some(child) = state.tunnel.as_mut() {
        if let Err(error) = stop_tunnel_child(child).await {
            let _ = gateway_control("/admin/resume", Some(json!({}))).await; return Err(error);
        }
    }
    state.tunnel = None;
    if restore_route {
        if let Err(error) = unified::set_route(&crate::relay_config::default_codex_home_dir(), false, GATEWAY_PORT) {
            if let Some(spec) = &state.gateway_spec { state.tunnel = start_tunnel(spec).ok().flatten(); }
            let _ = gateway_control("/admin/resume", Some(json!({}))).await; return Err(error);
        }
    }
    state.enabled = false;
    if gateway_alive { let _ = gateway_control("/admin/shutdown", Some(json!({}))).await; }
    if let Some(mut child) = state.gateway.take() {
        if tokio::time::timeout(Duration::from_secs(5), child.wait()).await.is_err() { child.kill().await?; }
    }
    if browser_alive { let _ = browser_control_for_pid("quit", state.browser.as_ref().and_then(Child::id)).await; }
    if let Some(mut child) = state.browser.take() {
        if tokio::time::timeout(Duration::from_secs(5), child.wait()).await.is_err() { child.kill().await?; }
    }
    Ok(())
}

pub async fn show_login() -> anyhow::Result<Value> {
    browser_control("show").await
}

async fn browser_control(action: &str) -> anyhow::Result<Value> {
    let expected_pid = runtime().lock().await.browser.as_ref().and_then(Child::id);
    browser_control_for_pid(action, expected_pid).await
}

async fn browser_control_for_pid(action: &str, expected_pid: Option<u32>) -> anyhow::Result<Value> {
    let descriptor: Value = serde_json::from_slice(&std::fs::read(unified::profile_dir().join("runtime/launcher-browser.json"))?)?;
    let endpoint = descriptor["control"]["endpoint"].as_str().context("Browser control endpoint missing")?;
    let token = descriptor["control"]["token"].as_str().context("Browser control credential missing")?;
    let url = reqwest::Url::parse(endpoint)?;
    if url.scheme() != "http" || url.host_str() != Some("127.0.0.1") || !url.username().is_empty() || url.password().is_some() { bail!("Invalid browser control endpoint"); }
    if expected_pid.is_none() || descriptor["pid"].as_u64() != expected_pid.map(u64::from) { bail!("Browser descriptor belongs to a different process"); }
    Ok(local_client()?.post(format!("{}/v1/managed/{action}", endpoint.trim_end_matches('/'))).bearer_auth(token).send().await?.error_for_status()?.json().await?)
}

pub async fn configure_web(value: Value) -> anyhow::Result<Value> {
    prepare_web_change().await?;
    let result = configure_web_inner(value).await;
    let _ = gateway_control("/admin/web-resume", Some(json!({}))).await;
    result
}

async fn prepare_web_change() -> anyhow::Result<()> {
    let current = gateway_control("/admin/web-drain", Some(json!({}))).await?;
    if current["active_http_turns"].as_u64().unwrap_or(1) > 0 || current["active_browser_turns"].as_u64().unwrap_or(1) > 0 {
        let _ = gateway_control("/admin/web-resume", Some(json!({}))).await;
        bail!("Finish active HTTP and Web requests before changing Web components");
    }
    Ok(())
}

async fn configure_web_inner(value: Value) -> anyhow::Result<Value> {
    let shared = runtime();
    let state = shared.lock().await;
    let spec = state.gateway_spec.as_ref().context("Unified gateway is not running")?.clone();
    drop(state);
    let profile = unified::profile_dir();
    let mut child = spec.command(&profile)?.args(["managed", "configure", "--config"]).arg(profile.join("managed-launch.json")).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
    let mut stdin = child.stdin.take().context("Managed configuration stdin unavailable")?;
    stdin.write_all(&serde_json::to_vec(&value)?).await?; drop(stdin);
    let result = tokio::time::timeout(Duration::from_secs(180), child.wait_with_output()).await??;
    if !result.status.success() { bail!("Web connection configuration failed; check the selected tunnel credentials and runtime paths"); }
    let result: Value = serde_json::from_slice(&result.stdout)?;
    let mut state = shared.lock().await;
    if let Some(child) = state.tunnel.as_mut() { stop_tunnel_child(child).await?; }
    state.tunnel = None;
    state.tunnel = start_tunnel(&spec)?;
    drop(state);
    let _ = browser_control("reload").await;
    Ok(result)
}

/// Invoked through the authenticated Codex renderer bridge, never through public HTTP CORS.
pub async fn command(payload: Value) -> anyhow::Result<Value> {
    let _operation = if payload["action"].as_str().unwrap_or("status") == "status" { None } else { Some(operations().lock().await) };
    match payload["action"].as_str().unwrap_or("status") {
        "status" => Ok(status().await),
        "import-components" => {
            if runtime().lock().await.enabled { bail!("Disable unified routing before changing components"); }
            let manifest_path = PathBuf::from(payload["path"].as_str().context("Component manifest path is required")?);
            if !manifest_path.is_absolute() || manifest_path.is_symlink() { bail!("Component manifest requires an absolute regular file"); }
            let manifest: Value = serde_json::from_slice(&std::fs::read(manifest_path)?)?;
            if manifest["schemaVersion"] != 1 || manifest["owner"] != "codex-plusplus" || manifest["ccwVersion"] != CCW_VERSION || manifest["bunVersion"] != "1.4.0" || manifest["electronVersion"] != "41.10.7" { bail!("Incompatible component manifest"); }
            for key in ["bunPath", "runtimeEntry", "electronPath", "browserEntry"] {
                let file = PathBuf::from(manifest["settings"][key].as_str().context("Component path missing")?);
                if !file.is_absolute() || format!("{:x}", Sha256::digest(std::fs::read(file)?)) != manifest["hashes"][key].as_str().context("Component digest missing")? { bail!("Component integrity check failed"); }
            }
            let store = SettingsStore::default(); let mut settings = store.load()?;
            let mut imported: unified::UnifiedSettings = serde_json::from_value(manifest["settings"].clone())?;
            imported.group_defaults = settings.unified.group_defaults.clone();
            settings.unified = imported; store.save(&settings)?;
            Ok(status().await)
        }
        "save-components" => {
            let store = SettingsStore::default();
            let mut settings = store.load()?;
            let updated: unified::UnifiedSettings = serde_json::from_value(payload["settings"].clone())?;
            if runtime().lock().await.enabled { bail!("Disable unified routing before changing component paths"); }
            settings.unified = updated; store.save(&settings)?;
            Ok(status().await)
        }
        "set-group-default" => {
            let store = SettingsStore::default(); let mut settings = store.load()?;
            let group_id = payload["groupId"].as_str().context("Group ID is required")?;
            let group = settings.aggregate_relay_profiles.iter().find(|group| group.id == group_id).context("Unknown aggregate group")?;
            if let Some(profile_id) = payload["profileId"].as_str() {
                if !group.members.iter().any(|member| member.relay_id == profile_id) { bail!("Default provider must belong to the aggregate"); }
                settings.unified.group_defaults.insert(group_id.into(), profile_id.into());
            } else { settings.unified.group_defaults.remove(group_id); }
            unified::public_manifest(&settings)?;
            store.save(&settings)?;
            Ok(json!({"status":"ok","providerApplies":"next_turn"}))
        }
        "enable" => {
            let store = SettingsStore::default();
            let previous = store.load()?;
            let mut settings = previous.clone(); settings.routing_mode = RoutingMode::Unified;
            store.save(&settings)?;
            if let Err(error) = ensure_started_inner(&settings, crate::protocol_proxy::DEFAULT_PROTOCOL_PROXY_PORT).await {
                store.save(&previous)?; return Err(error);
            }
            Ok(status().await)
        }
        "disable" => {
            stop_inner(true).await?;
            let store = SettingsStore::default(); let mut settings = store.load()?;
            settings.routing_mode = RoutingMode::Legacy; store.save(&settings)?;
            Ok(status().await)
        }
        "restart" => {
            stop_inner(true).await?;
            ensure_started_inner(&SettingsStore::default().load()?, crate::protocol_proxy::DEFAULT_PROTOCOL_PROXY_PORT).await?;
            Ok(status().await)
        }
        "login" => { show_login().await?; Ok(json!({"status":"ok"})) }
        "configure-web" => configure_web(payload["settings"].clone()).await,
        "browser-restart" => {
            prepare_web_change().await?;
            let result: anyhow::Result<()> = async {
                let shared = runtime(); let mut state = shared.lock().await;
                if let Some(mut child) = state.browser.take() { child.kill().await?; }
                let spec = state.browser_spec.as_ref().context("Web browser component is not configured")?.clone();
                state.browser = Some(start_browser(&spec)?); state.browser_restarts = 0;
                Ok(())
            }.await;
            let _ = gateway_control("/admin/web-resume", Some(json!({}))).await;
            result?;
            Ok(status().await)
        }
        _ => bail!("Unknown unified runtime action"),
    }
}
