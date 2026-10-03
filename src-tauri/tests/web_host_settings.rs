use serde_json::{json, Value};
use video_creater_lib::app_service::error::ServiceErrorCode;
use video_creater_lib::web_host::{
    dispatcher::HostDispatcher,
    rpc::{RpcDispatcher, RpcEnvelope},
};

fn request(operation: &str, payload: Value) -> RpcEnvelope {
    RpcEnvelope {
        request_id: "settings-regression".into(),
        operation: operation.into(),
        project_id: None,
        expected_revision: None,
        editor_lease_token: None,
        payload,
    }
}

#[test]
fn preferences_reject_invalid_dimensions_and_unknown_fields() {
    let root = tempfile::tempdir().unwrap();
    let dispatcher = HostDispatcher::default().with_settings_root(root.path().to_path_buf());
    for patch in [
        json!({"newProjectDefaults":{"width":3,"height":1080,"fps":30,"loudnessLufs":-14,"captions":"burn_in"}}),
        json!({"unknownSetting":true}),
    ] {
        let result =
            dispatcher.dispatch_typed(&request("update_app_preferences", json!({"patch":patch})));
        assert!(matches!(result, Err(error) if error.code() == ServiceErrorCode::InvalidInput));
    }
}

#[test]
fn model_readiness_uses_the_real_catalog_and_frontend_contracts() {
    let root = tempfile::tempdir().unwrap();
    let dispatcher = HostDispatcher::default().with_settings_root(root.path().to_path_buf());
    let models = dispatcher
        .dispatch_typed(&request("list_transcription_models", json!({})))
        .unwrap();
    assert!(!models.as_array().unwrap().is_empty());
    let active = dispatcher
        .dispatch_typed(&request("get_active_transcription_model", json!({})))
        .unwrap();
    assert!(active["modelId"].is_string());
    let runtime = dispatcher
        .dispatch_typed(&request("get_transcription_runtime_status", json!({})))
        .unwrap();
    assert!(matches!(
        runtime.as_str(),
        Some("native" | "unsupported_platform" | "unavailable")
    ));
    let speech = dispatcher
        .dispatch_typed(&request("get_production_speech_model_status", json!({})))
        .unwrap();
    assert!(speech["modelSetId"].is_string());
    assert!(speech["totalFiles"].as_u64().unwrap() > 0);
}

#[test]
fn settings_health_rejects_host_paths_before_running_probes() {
    let root = tempfile::tempdir().unwrap();
    let dispatcher = HostDispatcher::default().with_settings_root(root.path().to_path_buf());
    let result = dispatcher.dispatch_typed(&request(
        "get_settings_health_snapshot",
        json!({"projectRoot":"/etc"}),
    ));
    assert!(matches!(result, Err(error) if error.code() == ServiceErrorCode::InvalidInput));
}

#[test]
fn preferences_survive_dispatcher_restart_and_failed_updates_preserve_saved_bytes() {
    let root = tempfile::tempdir().unwrap();
    let dispatcher = HostDispatcher::default().with_settings_root(root.path().to_path_buf());
    let saved = dispatcher.dispatch_typed(&request("update_app_preferences", json!({"patch":{"newProjectDefaults":{"width":1920,"height":1080,"fps":24,"loudnessLufs":-16,"captions":"burn_in"}}}))).unwrap();
    assert_eq!(saved["newProjectDefaults"]["fps"], 24.0);
    let path = root.path().join("settings/preferences.json");
    let before = std::fs::read(&path).unwrap();
    let restarted = HostDispatcher::default().with_settings_root(root.path().to_path_buf());
    assert_eq!(
        restarted
            .dispatch_typed(&request("get_app_preferences", json!({})))
            .unwrap(),
        saved
    );
    assert!(restarted.dispatch_typed(&request("update_app_preferences", json!({"patch":{"newProjectDefaults":{"width":3,"height":1080,"fps":24,"loudnessLufs":-16,"captions":"burn_in"}}}))).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::write(&path, b"corrupt saved preferences").unwrap();
    assert!(restarted
        .dispatch_typed(&request("update_app_preferences", json!({"patch":{}})))
        .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"corrupt saved preferences");
}

#[test]
fn unsupported_models_and_invalid_credentials_do_not_touch_storage() {
    let root = tempfile::tempdir().unwrap();
    let dispatcher = HostDispatcher::default().with_settings_root(root.path().to_path_buf());
    for operation in [
        "download_transcription_model",
        "set_active_transcription_model",
        "verify_transcription_model",
        "remove_transcription_model",
        "cancel_model_download",
    ] {
        let result =
            dispatcher.dispatch_typed(&request(operation, json!({"modelId":"../../etc/passwd"})));
        assert!(
            matches!(result, Err(error) if error.code() == ServiceErrorCode::InvalidInput),
            "{operation}"
        );
    }
    for payload in [
        json!({"provider":"unsupported", "credential":"fixture"}),
        json!({"provider":"openai", "credential":" "}),
    ] {
        assert!(
            matches!(dispatcher.dispatch_typed(&request("set_provider_credential", payload)), Err(error) if error.code() == ServiceErrorCode::InvalidInput)
        );
    }
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn every_settings_health_scope_rejects_host_paths() {
    let root = tempfile::tempdir().unwrap();
    let dispatcher = HostDispatcher::default().with_settings_root(root.path().to_path_buf());
    for operation in [
        "get_settings_health_snapshot",
        "get_system_health_snapshot",
        "refresh_system_health_section",
        "get_agent_settings_health",
        "get_skills_settings_health",
        "get_storage_health",
        "get_render_system_health",
    ] {
        assert!(
            matches!(dispatcher.dispatch_typed(&request(operation, json!({"activeProjectDir":"/etc", "sectionId":"rendering"}))), Err(error) if error.code() == ServiceErrorCode::InvalidInput),
            "{operation}"
        );
    }
}

#[test]
fn native_host_path_actions_return_controlled_unavailable_without_reads() {
    let root = tempfile::tempdir().unwrap();
    let dispatcher = HostDispatcher::default().with_settings_root(root.path().to_path_buf());
    for operation in [
        "import_transcription_model",
        "get_notification_capability",
        "request_notification_permission",
        "preview_storage_cleanup",
        "run_storage_cleanup",
        "reveal_storage_inventory_item",
        "repair_bundled_skills",
        "refresh_provider_health",
    ] {
        assert!(
            matches!(dispatcher.dispatch_typed(&request(operation, json!({"sourcePath":"/etc", "activeProjectDir":"/etc", "provider":"openai"}))), Err(error) if error.code() == ServiceErrorCode::UnavailableCapability),
            "{operation}"
        );
    }
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
