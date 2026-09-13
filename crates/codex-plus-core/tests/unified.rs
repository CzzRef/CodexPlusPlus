use codex_plus_core::{settings::*, unified::*};
use serde_json::json;
use std::fs;
use tempfile::tempdir;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::{method, path, header, body_partial_json}};

fn api(id: &str, base: &str) -> RelayProfile {
    RelayProfile { id: id.into(), name: format!("Provider {id}"), relay_mode: RelayMode::PureApi,
        base_url: base.into(), api_key: format!("secret-{id}"), model: "model-a".into(),
        model_list: "model-a\nmodel-b".into(), model_windows: r#"{"model-a":"100K"}"#.into(),
        model_auto_compact: r#"{"model-a":"80"}"#.into(), ..RelayProfile::default() }
}

#[test]
fn manifest_is_credential_free_and_retains_scoped_model_capabilities() {
    let settings = BackendSettings { relay_profiles: vec![api("a", "https://example.test/v1"), api("b", "https://other.test/v1")],
        aggregate_relay_profiles: vec![AggregateRelayProfile { id: "group".into(), name: "Group".into(),
            session_provider: RelaySessionProvider::Openai, strategy: AggregateRelayStrategy::RequestRoundRobin,
            members: vec![AggregateRelayMember { relay_id: "a".into(), weight: 1 }, AggregateRelayMember { relay_id: "b".into(), weight: 2 }],
            routes: vec![] }],
        ..BackendSettings::default() };
    let manifest = public_manifest(&settings).unwrap();
    let serialized = manifest.to_string();
    for forbidden in ["secret-", "example.test", "other.test", "authContents", "configContents"] { assert!(!serialized.contains(forbidden)); }
    let row = manifest["models"].as_array().unwrap().iter().find(|r| r["slug"] == "cpp/a/model-a").unwrap();
    assert_eq!(row["context_window"], 100_000);
    assert_eq!(row["auto_compact_token_limit"], 80_000);
    assert!(manifest["models"].as_array().unwrap().iter().any(|r| r["slug"] == "cpp-agg/group/model-a"));
    assert_eq!(public_manifest(&settings).unwrap()["revision"], manifest["revision"]);
}

#[test]
fn route_transaction_preserves_auth_voice_and_unrelated_edits_on_restore() {
    let home = tempdir().unwrap();
    fs::write(home.path().join("config.toml"), "# user settings\nmodel_provider = \"custom\"\n[features]\nvoice = true\nmulti_agent = true\n").unwrap();
    fs::write(home.path().join("auth.json"), "native-auth-unchanged").unwrap();
    set_route(home.path(), true, 17841).unwrap();
    let file = home.path().join("config.toml");
    let active = fs::read_to_string(&file).unwrap();
    assert!(active.contains("http://127.0.0.1:17841/v1"));
    assert!(active.contains("model_provider = \"openai\""));
    assert!(active.contains("experimental_realtime_webrtc_call_base_url = \"https://chatgpt.com/backend-api/codex\""));
    assert!(guard_legacy_writer(home.path()).is_err());
    fs::write(&file, format!("{active}\n[projects]\ntrust = true\n")).unwrap();
    set_route(home.path(), false, 17841).unwrap();
    let restored = fs::read_to_string(&file).unwrap();
    assert!(restored.contains("model_provider = \"custom\""));
    assert!(restored.contains("voice = true") && restored.contains("multi_agent = true") && restored.contains("trust = true"));
    assert!(!restored.contains("openai_base_url"));
    assert!(!restored.contains("experimental_realtime_webrtc_call_base_url"));
    assert_eq!(fs::read_to_string(home.path().join("auth.json")).unwrap(), "native-auth-unchanged");
}

#[test]
fn route_transaction_refuses_external_conflicts_and_writer_takeover() {
    let home = tempdir().unwrap();
    set_route(home.path(), true, 17841).unwrap();
    let profile = api("api", "https://example.test");
    assert!(codex_plus_core::relay_config::apply_relay_profile_to_home_with_switch_rules(home.path(), &profile, "").is_err());
    let file = home.path().join("config.toml");
    let changed = fs::read_to_string(&file).unwrap().replace("17841", "19000");
    fs::write(&file, &changed).unwrap();
    assert!(set_route(home.path(), false, 17841).is_err());
    assert_eq!(fs::read_to_string(file).unwrap(), changed);
}

#[test]
fn legacy_is_default_and_unknown_targets_do_not_use_active_profile() {
    assert_eq!(serde_json::from_value::<BackendSettings>(json!({})).unwrap().routing_mode, RoutingMode::Legacy);
    let settings = BackendSettings { relay_profiles: vec![api("a", "https://example.test")], active_relay_id: "a".into(), ..BackendSettings::default() };
    assert!(explicit_profile(&settings, "missing", "model-a").is_err());
    assert_eq!(explicit_profile(&settings, "a", "model-b").unwrap().model, "model-b");
}

#[tokio::test]
async fn explicit_executor_uses_selected_profile_auth_and_preserves_turn_identity() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/v1/responses"))
        .and(header("authorization", "Bearer secret-b"))
        .and(body_partial_json(json!({"model":"model-b", "client_metadata":{"marker":"preserved"}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"completed","output":[]}))).expect(1).mount(&server).await;
    let settings = BackendSettings { relay_profiles: vec![api("a", "http://127.0.0.1:1/v1"), api("b", &format!("{}/v1", server.uri()))], active_relay_id: "a".into(), ..BackendSettings::default() };
    let response = codex_plus_core::protocol_proxy::open_explicit_responses_request(
        &json!({"model":"model-b","input":[],"client_metadata":{"marker":"preserved"}}).to_string(), &settings, "b", None,
        &[("authorization".into(), "Bearer native-must-not-leak".into()), ("x-codex-turn-metadata".into(), r#"{"thread_id":"t","turn_id":"u"}"#.into())],
    ).await.unwrap();
    assert_eq!(response.status_code, 200);
    assert_eq!(settings.active_relay_id, "a");
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests[0].headers["x-codex-turn-metadata"].to_str().unwrap(), r#"{"thread_id":"t","turn_id":"u"}"#);
}

#[tokio::test]
async fn explicit_chat_completions_executor_converts_tool_results() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"choices":[]}))).expect(1).mount(&server).await;
    let mut profile = api("b", &server.uri()); profile.protocol = RelayProtocol::ChatCompletions;
    let settings = BackendSettings { relay_profiles: vec![profile], ..BackendSettings::default() };
    let response = codex_plus_core::protocol_proxy::open_explicit_responses_request(
        &json!({"model":"model-a","input":[{"type":"function_call","call_id":"call_1","name":"read_file","arguments":"{}"},{"type":"function_call_output","call_id":"call_1","output":"result"}]}).to_string(), &settings, "b", None, &[],
    ).await.unwrap();
    assert_eq!(response.status_code, 200);
    let requests = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["messages"][0]["tool_calls"][0]["function"]["name"], "read_file");
    assert_eq!(body["messages"][1], json!({"role":"tool","tool_call_id":"call_1","content":"result"}));
}

#[test]
fn in_flight_revision_retains_the_original_provider_configuration() {
    let mut settings = BackendSettings { relay_profiles: vec![api("a", "https://old.test/v1")], ..BackendSettings::default() };
    let old = public_manifest(&settings).unwrap();
    settings.relay_profiles[0].base_url = "https://new.test/v1".into();
    let new = public_manifest(&settings).unwrap();
    assert_ne!(old["revision"], new["revision"]);
    let snapshot = settings_for_revision(old["revision"].as_str().unwrap()).unwrap();
    assert_eq!(explicit_profile(&snapshot, "a", "model-a").unwrap().base_url, "https://old.test/v1");
}

#[tokio::test]
async fn cancellation_drops_active_requests_and_rejects_early_cancelled_ids() {
    let id = uuid::Uuid::new_v4().to_string();
    let (_guard, registration) = register_request(&id).unwrap();
    cancel_request(&id).unwrap();
    assert!(futures_util::future::Abortable::new(std::future::pending::<()>(), registration).await.is_err());
    let early = uuid::Uuid::new_v4().to_string(); cancel_request(&early).unwrap();
    assert!(register_request(&early).is_err());
}

#[test]
fn managed_interrupt_hook_restores_without_removing_other_hooks() {
    let home = tempdir().unwrap();
    let original = "[[hooks.Interrupt]]\n[[hooks.Interrupt.hooks]]\ntype = \"command\"\ncommand = \"user-hook\"\n";
    fs::write(home.path().join("config.toml"), original).unwrap();
    set_route_with_hook(home.path(), true, 17841, Some("'/runtime path/bun' '/runtime/cli.js' managed interrupt")).unwrap();
    let active = fs::read_to_string(home.path().join("config.toml")).unwrap();
    assert!(active.contains("trusted_hash = \"sha256:"));
    assert!(active.contains("user-hook"));
    set_route(home.path(), false, 17841).unwrap();
    let restored = fs::read_to_string(home.path().join("config.toml")).unwrap();
    assert!(restored.contains("user-hook"));
    assert!(!restored.contains("managed Interrupt") && !restored.contains("trusted_hash"));
}

#[test]
fn legacy_cpp_catalog_and_endpoint_can_be_migrated_and_restored() {
    let home = tempdir().unwrap();
    let file = home.path().join("config.toml");
    let original = "openai_base_url = \"http://127.0.0.1:57321/v1\"\nmodel_catalog_json = \"model-catalogs/api.json\"\n";
    fs::write(&file, original).unwrap();
    assert!(set_route(home.path(), true, 17841).is_err());
    set_route_with_legacy_catalogs(home.path(), true, 17841, None, &[home.path().join("model-catalogs/api.json")]).unwrap();
    set_route(home.path(), false, 17841).unwrap();
    let restored = fs::read_to_string(file).unwrap();
    assert!(restored.contains("57321") && restored.contains("model-catalogs/api.json"));
}

#[test]
fn interrupted_route_activation_recovers_before_applying_the_same_contract() {
    let home = tempdir().unwrap();
    let file = home.path().join("config.toml");
    let original = "# preserve this\n[features]\nvoice = true\n";
    fs::write(&file, original).unwrap();
    set_route(home.path(), true, 17841).unwrap();
    // Simulate a crash after the journal write but before the config rename.
    fs::write(&file, original).unwrap();
    set_route(home.path(), true, 17841).unwrap();
    assert!(fs::read_to_string(&file).unwrap().contains("17841"));
    set_route(home.path(), false, 17841).unwrap();
    assert!(fs::read_to_string(file).unwrap().contains("voice = true"));
}

#[test]
fn global_profile_switch_cannot_write_settings_or_live_files_in_unified_mode() {
    let home = tempdir().unwrap();
    let store = SettingsStore::new(home.path().join("settings.json"));
    let settings = BackendSettings { routing_mode: RoutingMode::Unified, relay_profiles: vec![api("a", "https://example.test")], ..BackendSettings::default() };
    store.save(&settings).unwrap();
    fs::write(home.path().join("config.toml"), "# unchanged\n").unwrap();
    let before = fs::read(home.path().join("settings.json")).unwrap();
    let mut stale = settings; stale.routing_mode = RoutingMode::Legacy;
    assert!(codex_plus_core::relay_switch::switch_relay_profile_in_home(&store, home.path(), stale, "").is_err());
    assert_eq!(fs::read(home.path().join("settings.json")).unwrap(), before);
    assert_eq!(fs::read_to_string(home.path().join("config.toml")).unwrap(), "# unchanged\n");
}

#[test]
fn native_context_moves_to_catalog_contract_and_cache_is_invalidated_reversibly() {
    let home = tempdir().unwrap();
    let file = home.path().join("config.toml");
    fs::write(&file, "model_context_window = 100000\nmodel_auto_compact_token_limit = 80000\n").unwrap();
    fs::write(home.path().join("models_cache.json"), "fixture cached catalog").unwrap();
    let context = native_context_override(home.path()).unwrap();
    assert_eq!(context, json!({"contextWindow":100000,"autoCompactTokenLimit":80000}));
    set_route(home.path(), true, 17841).unwrap();
    assert!(!fs::read_to_string(&file).unwrap().contains("model_context_window"));
    assert!(!home.path().join("models_cache.json").exists());
    let backup = fs::read_dir(home.path().join("codex-plus-cache-backups")).unwrap().next().unwrap().unwrap().path();
    assert_eq!(fs::read_to_string(backup).unwrap(), "fixture cached catalog");
    assert_eq!(native_context_override(home.path()).unwrap(), context);
    set_route(home.path(), false, 17841).unwrap();
    assert!(fs::read_to_string(file).unwrap().contains("model_context_window = 100000"));
}
