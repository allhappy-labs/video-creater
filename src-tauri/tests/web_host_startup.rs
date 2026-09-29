use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use video_creater_lib::web_host::config::HostConfig;

#[test]
fn host_defaults_to_loopback_and_production_assets() {
    let config = HostConfig::default();
    assert_eq!(config.bind.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
    assert_eq!(config.assets_dir, PathBuf::from("dist"));
    assert!(!config.allow_insecure_direct_bind);
}

#[test]
fn non_loopback_bind_fails_closed_without_explicit_developer_override() {
    let denied = HostConfig {
        bind: SocketAddr::from(([0, 0, 0, 0], 4777)),
        ..HostConfig::default()
    };
    assert!(denied.validate().is_err());

    let allowed = HostConfig {
        allow_insecure_direct_bind: true,
        ..denied
    };
    assert!(allowed.validate().is_ok());
    assert!(allowed.insecure_bind_warning().is_some());
}

#[test]
fn health_payload_contains_compatibility_but_no_host_data() {
    let health = video_creater_lib::web_host::health_payload();
    let json = serde_json::to_value(health).expect("health payload serializes");
    assert_eq!(json["status"], "ok");
    assert!(json["protocolVersion"].is_number());
    let serialized = json.to_string();
    for forbidden in ["project", "path", "user", "secret", "token"] {
        assert!(
            !serialized.to_ascii_lowercase().contains(forbidden),
            "{serialized}"
        );
    }
}
