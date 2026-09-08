//! Codex++ is the configuration owner. The managed CCW component receives only
//! a public routing manifest and a private local control credential.
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::collections::VecDeque;

use anyhow::{Context, bail};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use toml_edit::{DocumentMut, Item};

use crate::settings::{BackendSettings, RelayMode, RelayProfile};

pub const CONTRACT_VERSION: u32 = 1;
pub const CCW_VERSION: &str = "5.0.5";
pub const GATEWAY_PORT: u16 = 17841;
const JOURNAL: &str = "codex-plus-unified-route.json";
const OWNED_KEYS: &[&str] = &["model_provider", "openai_base_url", "model_catalog_json", "model_context_window", "model_auto_compact_token_limit", "experimental_realtime_webrtc_call_base_url"];
const NATIVE_VOICE_BASE: &str = "https://chatgpt.com/backend-api/codex";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoutingMode {
    #[default]
    Legacy,
    Unified,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UnifiedSettings {
    pub bun_path: String,
    pub runtime_entry: String,
    pub electron_path: String,
    pub browser_entry: String,
    pub group_defaults: BTreeMap<String, String>,
}

pub fn profile_dir() -> PathBuf {
    crate::paths::default_app_state_dir().join("integrations/chatgpt-web")
}

pub fn public_manifest(settings: &BackendSettings) -> anyhow::Result<Value> {
    let mut profiles = Vec::new();
    let mut models = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for source in &settings.relay_profiles {
        if source.relay_mode == RelayMode::Aggregate { continue; }
        let base = crate::relay_config::relay_profile_base_url(source);
        let key = crate::relay_config::relay_profile_api_key(source);
        // An official-only profile has no independent API route.
        if base.is_empty() || (key.is_empty() && !source.uses_no_auth()) { continue; }
        validate_id(&source.id)?;
        if !seen.insert(source.id.clone()) { bail!("Duplicate profile id"); }
        let windows: HashMap<String, String> = parse_map(&source.model_windows)?;
        let compact: HashMap<String, String> = parse_map(&source.model_auto_compact)?;
        let metadata: HashMap<String, Value> = parse_map(&source.model_metadata)?;
        let entries = crate::model_suffix::collect_catalog_entries(
            &source.model_list, &windows, &compact, &crate::relay_config::relay_profile_model(source),
        );
        let catalog: Value = serde_json::from_str(
            &crate::model_suffix::build_model_catalog_json_with_capabilities(
                &entries, source.context_window.parse().ok(), None, Some(false), false,
            ),
        )?;
        let mut ids = Vec::new();
        for row in catalog["models"].as_array().into_iter().flatten() {
            let mut row = row.clone();
            let upstream_model = row["slug"].as_str().context("Missing catalog slug")?.to_string();
            if row["auto_compact_token_limit"].is_null() {
                if let Ok(limit) = source.auto_compact_limit.parse::<u64>() { if limit > 0 { row["auto_compact_token_limit"] = json!(limit); } }
            }
            if let Some(overrides) = metadata.get(&upstream_model).and_then(Value::as_object) {
                // Metadata is user authored. Only model capability fields are public.
                for field in ["display_name", "description", "supported_reasoning_levels", "default_reasoning_level", "context_window", "max_context_window", "auto_compact_token_limit", "input_modalities", "supports_reasoning_summaries", "supports_parallel_tool_calls"] {
                    if let Some(value) = overrides.get(field) { row[field] = value.clone(); }
                }
            }
            let slug = format!("cpp/{}/{}", source.id, upstream_model);
            row["slug"] = json!(slug);
            row["display_name"] = json!(format!("{} · {}", source.name, row["display_name"].as_str().unwrap_or(&upstream_model)));
            row["cpp_route"] = json!({"kind":"api", "profileId":source.id, "model":upstream_model});
            // Both API protocols use the managed tools-disabled compaction adapter.
            row["use_responses_lite"] = json!(false);
            ids.push(slug);
            models.push(row);
        }
        let fingerprint = format!("{:x}", Sha256::digest(serde_json::to_vec(&json!({"profile": source, "effectiveBase": base, "credential": key, "routes": source.model_routes.iter().map(|route| settings.relay_profiles.iter().find(|profile| profile.id == route.target_relay_id)).collect::<Vec<_>>() }))?));
        profiles.push(json!({"id":source.id,"name":source.name,"protocol":source.protocol,"models":ids,"revision":fingerprint}));
    }
    let mut groups = Vec::new();
    for group in &settings.aggregate_relay_profiles {
        validate_id(&group.id)?;
        if group.members.is_empty() { continue; }
        let mut common: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for member in &group.members {
            if !seen.contains(&member.relay_id) { bail!("Aggregate {} has an unavailable API member {}", group.id, member.relay_id); }
            for row in &models {
                if row["cpp_route"]["profileId"].as_str() == Some(&member.relay_id) {
                    common.entry(row["cpp_route"]["model"].as_str().unwrap().to_string()).or_default().push(row.clone());
                }
            }
        }
        // A group only advertises models executable by every configured member.
        for (model, rows) in common {
            if rows.len() != group.members.len() { continue; }
            let mut row = rows[0].clone();
            row["slug"] = json!(format!("cpp-agg/{}/{}", group.id, model));
            row["display_name"] = json!(format!("{} · {}", group.name, model));
            row["cpp_route"] = json!({"kind":"aggregate","groupId":group.id,"model":model});
            for field in ["context_window", "max_context_window", "auto_compact_token_limit"] {
                if let Some(limit) = rows.iter().filter_map(|r| r[field].as_u64()).min() { row[field] = json!(limit); }
            }
            if let Some(levels) = rows[0]["supported_reasoning_levels"].as_array() {
                row["supported_reasoning_levels"] = json!(levels.iter().filter(|level| rows.iter().all(|r| r["supported_reasoning_levels"].as_array().is_some_and(|list| list.iter().any(|v| effort_name(v) == effort_name(level))))) .collect::<Vec<_>>());
            }
            if let Some(levels) = row["supported_reasoning_levels"].as_array() {
                if !levels.iter().any(|level| effort_name(level) == row["default_reasoning_level"].as_str()) {
                    row["default_reasoning_level"] = json!(levels.first().and_then(effort_name));
                }
            }
            models.push(row);
        }
        groups.push(json!({"id":group.id,"name":group.name,"strategy":group.strategy,"members":group.members,"defaultProfileId":settings.unified.group_defaults.get(&group.id)}));
    }
    let mut manifest = json!({"schemaVersion":CONTRACT_VERSION,"ccwVersion":CCW_VERSION,"models":models,"profiles":profiles,"groups":groups});
    let revision = format!("{:x}", Sha256::digest(serde_json::to_vec(&manifest)?));
    manifest["revision"] = json!(revision);
    let mut snapshots = snapshots().lock().map_err(|_| anyhow::anyhow!("Unified snapshot lock poisoned"))?;
    if !snapshots.iter().any(|(id, _)| id == &revision) {
        if snapshots.len() >= 256 { snapshots.pop_front(); }
        snapshots.push_back((revision, Arc::new(settings.clone())));
    }
    Ok(manifest)
}

type SnapshotCache = Mutex<VecDeque<(String, Arc<BackendSettings>)>>;
fn effort_name(value: &Value) -> Option<&str> { value.as_str().or_else(|| value.get("effort").and_then(Value::as_str)) }
fn snapshots() -> &'static SnapshotCache {
    static SNAPSHOTS: OnceLock<SnapshotCache> = OnceLock::new();
    SNAPSHOTS.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub fn settings_for_revision(revision: &str) -> anyhow::Result<Arc<BackendSettings>> {
    snapshots().lock().map_err(|_| anyhow::anyhow!("Unified snapshot lock poisoned"))?.iter()
        .find(|(id, _)| id == revision).map(|(_, settings)| settings.clone())
        .context("The bound provider revision is unavailable; start a new turn after refreshing the model list")
}

#[derive(Default)]
struct RequestRegistry {
    active: HashMap<String, futures_util::future::AbortHandle>,
    cancelled: HashMap<String, std::time::Instant>,
}

fn requests() -> &'static Mutex<RequestRegistry> {
    static REQUESTS: OnceLock<Mutex<RequestRegistry>> = OnceLock::new();
    REQUESTS.get_or_init(|| Mutex::new(RequestRegistry::default()))
}

pub fn active_request_count() -> anyhow::Result<usize> {
    Ok(requests().lock().map_err(|_| anyhow::anyhow!("Unified request lock poisoned"))?.active.len())
}

pub struct RequestGuard(String);
impl Drop for RequestGuard {
    fn drop(&mut self) { if let Ok(mut registry) = requests().lock() { registry.active.remove(&self.0); } }
}

pub fn register_request(id: &str) -> anyhow::Result<(RequestGuard, futures_util::future::AbortRegistration)> {
    uuid::Uuid::parse_str(id).context("Managed API request ID is invalid")?;
    let mut registry = requests().lock().map_err(|_| anyhow::anyhow!("Managed request registry unavailable"))?;
    registry.cancelled.retain(|_, at| at.elapsed().as_secs() < 3600);
    if registry.cancelled.contains_key(id) { bail!("Managed request was already cancelled"); }
    if registry.active.len() >= 1024 || registry.active.contains_key(id) { bail!("Managed request ID is busy"); }
    let (abort, registration) = futures_util::future::AbortHandle::new_pair();
    registry.active.insert(id.into(), abort);
    Ok((RequestGuard(id.into()), registration))
}

pub fn cancel_request(id: &str) -> anyhow::Result<()> {
    uuid::Uuid::parse_str(id).context("Managed API request ID is invalid")?;
    let mut registry = requests().lock().map_err(|_| anyhow::anyhow!("Managed request registry unavailable"))?;
    if let Some(abort) = registry.active.get(id) { abort.abort(); }
    registry.cancelled.retain(|_, at| at.elapsed().as_secs() < 3600);
    if registry.cancelled.len() >= 20_000 { bail!("Managed cancellation registry is full"); }
    registry.cancelled.insert(id.into(), std::time::Instant::now());
    Ok(())
}

fn parse_map<T: serde::de::DeserializeOwned + Default>(value: &str) -> anyhow::Result<T> {
    if value.trim().is_empty() { Ok(T::default()) } else { Ok(serde_json::from_str(value)?) }
}

fn validate_id(value: &str) -> anyhow::Result<()> {
    if value.is_empty() || value.len() > 128 || !value.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)) { bail!("Invalid stable routing id"); }
    Ok(())
}

pub fn explicit_profile(settings: &BackendSettings, id: &str, model: &str) -> anyhow::Result<RelayProfile> {
    validate_id(id)?;
    let mut profile = settings.relay_profiles.iter().find(|p| p.id == id && p.relay_mode != RelayMode::Aggregate).context("Unknown explicit API profile")?.clone();
    // Profile routes stay profile-local and resolve once. Never consult activeRelayId.
    if let Some(route) = profile.model_routes.iter().find(|r| r.model == model) {
        let target = settings.relay_profiles.iter().find(|p| p.id == route.target_relay_id && p.relay_mode != RelayMode::Aggregate).context("Unknown model route target")?;
        let target_model = if route.target_model.is_empty() { model } else { &route.target_model }.to_string();
        profile = target.clone();
        profile.model = target_model;
    } else { profile.model = model.to_string(); }
    profile.base_url = crate::relay_config::relay_profile_base_url(&profile);
    profile.api_key = crate::relay_config::relay_profile_api_key(&profile);
    let url = reqwest::Url::parse(&profile.base_url).context("Invalid API base URL")?;
    if !matches!(url.scheme(), "http" | "https") || !url.username().is_empty() || url.password().is_some() { bail!("Invalid API endpoint"); }
    if url.host_str().is_some_and(|h| matches!(h, "localhost" | "127.0.0.1" | "[::1]")) && matches!(url.port_or_known_default(), Some(17841 | 57321)) { bail!("API endpoint loops back into unified routing"); }
    if profile.api_key.is_empty() && !profile.uses_no_auth() { bail!("API profile has no credential"); }
    Ok(profile)
}

#[derive(Serialize, Deserialize)]
struct RouteJournal {
    schema_version: u32,
    before: BTreeMap<String, Option<String>>,
    installed: BTreeMap<String, Option<String>>,
    #[serde(default)]
    interrupt_hook: Option<OwnedInterruptHook>,
}

#[derive(Serialize, Deserialize)]
struct OwnedInterruptHook { command: String, fragment: String }

/// Preserve the native context override without applying it to Web or API catalog rows.
pub fn native_context_override(home: &Path) -> anyhow::Result<Value> {
    let document: DocumentMut = fs::read_to_string(home.join("config.toml")).unwrap_or_default().parse()?;
    let saved = if home.join(JOURNAL).exists() { Some(serde_json::from_slice::<RouteJournal>(&fs::read(home.join(JOURNAL))?)?) } else { None };
    let mut output = json!({});
    for (key, field) in [("model_context_window", "contextWindow"), ("model_auto_compact_token_limit", "autoCompactTokenLimit")] {
        let value = if let Some(journal) = &saved {
            journal.before.get(key).and_then(|value| value.as_ref()).and_then(|value| format!("value = {value}\n").parse::<DocumentMut>().ok()).and_then(|value| value["value"].as_integer())
        } else { document.get(key).and_then(Item::as_integer) };
        if let Some(value) = value.filter(|value| *value > 0) { output[field] = json!(value); }
    }
    Ok(output)
}

fn invalidate_model_cache(home: &Path) -> anyhow::Result<()> {
    let cache = home.join("models_cache.json");
    if cache.is_symlink() { bail!("Model cache is a symlink; refusing to move an external cache"); }
    if cache.exists() {
        let backup_dir = home.join("codex-plus-cache-backups");
        if backup_dir.is_symlink() { bail!("Model cache backup directory is a symlink"); }
        fs::create_dir_all(&backup_dir)?;
        fs::rename(&cache, backup_dir.join(format!("{}.json", uuid::Uuid::new_v4())))?;
    }
    Ok(())
}

fn interrupt_hook(doc: &DocumentMut, home: &Path, command: &str) -> anyhow::Result<OwnedInterruptHook> {
    let index = doc.get("hooks").and_then(|hooks| hooks.get("Interrupt")).and_then(Item::as_array_of_tables).map_or(0, |groups| groups.len());
    let config_path = home.canonicalize()?.join("config.toml");
    let state_key = format!("{}:interrupt:{index}:0", config_path.display());
    let identity = json!({"event_name":"interrupt","hooks":[{"type":"command","command":command,"timeout":3,"async":false}]});
    let hash = format!("sha256:{:x}", Sha256::digest(serde_json::to_vec(&identity)?));
    let fragment = format!("\n# Codex++ managed Interrupt hook\n[[hooks.Interrupt]]\n[[hooks.Interrupt.hooks]]\ntype = \"command\"\ncommand = {}\ntimeout = 3\n[hooks.state.{}]\ntrusted_hash = {}\n# End Codex++ managed Interrupt hook\n", serde_json::to_string(command)?, serde_json::to_string(&state_key)?, serde_json::to_string(&hash)?);
    Ok(OwnedInterruptHook { command: command.into(), fragment })
}

fn values(doc: &DocumentMut) -> BTreeMap<String, Option<String>> {
    OWNED_KEYS.iter().map(|key| ((*key).to_string(), doc.get(key).map(|item| item.to_string().trim().to_string()))).collect()
}

/// Atomically replace a private file in the same filesystem; never follow a target symlink.
pub fn write_private(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if path.is_symlink() { bail!("Refusing to replace a symlink"); }
    let parent = path.parent().context("File has no parent")?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".cpp-{}.tmp", uuid::Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let result = (|| -> anyhow::Result<()> {
        let mut file = options.open(&temp)?;
        file.write_all(bytes)?; file.sync_all()?;
        fs::rename(&temp, path)?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(temp); }
    result
}

pub fn route_is_owned(home: &Path) -> bool { home.join(JOURNAL).exists() }

pub fn guard_legacy_writer(home: &Path) -> anyhow::Result<()> {
    if route_is_owned(home) { bail!("Unified routing owns Codex configuration; disable it before applying a legacy profile"); }
    Ok(())
}

pub fn set_route(home: &Path, enable: bool, port: u16) -> anyhow::Result<()> {
    set_route_with_hook(home, enable, port, None)
}

pub fn set_route_with_hook(home: &Path, enable: bool, port: u16, command: Option<&str>) -> anyhow::Result<()> {
    set_route_with_legacy_catalogs(home, enable, port, command, &[])
}

pub fn set_route_with_legacy_catalogs(home: &Path, enable: bool, port: u16, command: Option<&str>, known_catalogs: &[PathBuf]) -> anyhow::Result<()> {
    if port == 0 { bail!("Gateway port cannot be zero"); }
    fs::create_dir_all(home)?;
    let lock_path = home.join("codex-plus-unified.lock");
    if lock_path.is_symlink() { bail!("Route lock is a symlink"); }
    let lock = OpenOptions::new().create(true).truncate(false).read(true).write(true).open(lock_path)?;
    lock.lock_exclusive()?;
    let config_path = home.join("config.toml");
    if config_path.is_symlink() || home.join(JOURNAL).is_symlink() { bail!("Route files must not be symlinks"); }
    let mut original = match fs::read_to_string(&config_path) { Ok(s) => s, Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(), Err(e) => return Err(e.into()) };
    let mut doc: DocumentMut = original.parse()?;
    let journal_path = home.join(JOURNAL);
    if journal_path.exists() {
        let journal: RouteJournal = serde_json::from_slice(&fs::read(&journal_path)?)?;
        if journal.schema_version != CONTRACT_VERSION { bail!("Unsupported route journal version"); }
        let current = values(&doc);
        // A crash before the config rename leaves the original state intact.
        if current != journal.installed && current != journal.before { bail!("Owned route keys changed externally; refusing to overwrite"); }
        if enable && current == journal.installed {
            if journal.interrupt_hook.as_ref().map(|hook| hook.command.as_str()) != command { bail!("Installed interrupt hook belongs to a different component; disable before upgrading"); }
            return Ok(());
        }
        if let Some(hook) = &journal.interrupt_hook {
            if current == journal.installed {
                if original.matches(&hook.fragment).count() != 1 { bail!("Owned interrupt hook changed externally; refusing to overwrite"); }
                doc = original.replacen(&hook.fragment, "", 1).parse()?;
            }
        }
        for (key, value) in &journal.before {
            if let Some(value) = value { let restored: DocumentMut = format!("{key} = {value}\n").parse()?; doc[key] = restored[key].clone(); }
            else { doc.remove(key); }
        }
        if fs::read_to_string(&config_path).unwrap_or_default() != original { bail!("Codex config changed during route transaction"); }
        invalidate_model_cache(home)?;
        write_private(&config_path, doc.to_string().as_bytes())?;
        fs::remove_file(&journal_path)?;
        if !enable { return Ok(()); }
        original = doc.to_string();
    } else if !enable { return Ok(()); }
    if let Some(existing) = doc.get("openai_base_url").and_then(Item::as_str) {
        if !existing.is_empty() && existing != "http://127.0.0.1:57321/v1" { bail!("openai_base_url is already owned by another integration"); }
    }
    if let Some(catalog) = doc.get("model_catalog_json").and_then(Item::as_str) {
        let catalog_path = if Path::new(catalog).is_absolute() { PathBuf::from(catalog) } else { home.join(catalog) };
        if !known_catalogs.contains(&catalog_path) { bail!("An external model catalog is installed; resolve it before unified activation"); }
    }
    if let Some(voice) = doc.get("experimental_realtime_webrtc_call_base_url").and_then(Item::as_str) {
        if voice != NATIVE_VOICE_BASE { bail!("Voice routing is owned by another integration"); }
    }
    let before = values(&doc);
    doc["model_provider"] = toml_edit::value("openai");
    doc["openai_base_url"] = toml_edit::value(format!("http://127.0.0.1:{port}/v1"));
    doc["experimental_realtime_webrtc_call_base_url"] = toml_edit::value(NATIVE_VOICE_BASE);
    doc.remove("model_catalog_json");
    doc.remove("model_context_window");
    doc.remove("model_auto_compact_token_limit");
    let hook = command.map(|command| interrupt_hook(&doc, home, command)).transpose()?;
    let installed_text = format!("{}{}", doc, hook.as_ref().map(|hook| hook.fragment.as_str()).unwrap_or(""));
    let journal = RouteJournal { schema_version: CONTRACT_VERSION, before, installed: values(&doc), interrupt_hook: hook };
    invalidate_model_cache(home)?;
    write_private(&journal_path, &serde_json::to_vec_pretty(&journal)?)?;
    if fs::read_to_string(&config_path).unwrap_or_default() != original { bail!("Codex config changed during route transaction; recovery journal retained"); }
    write_private(&config_path, installed_text.as_bytes())
}
