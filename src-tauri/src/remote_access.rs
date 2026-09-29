use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Command;
use std::time::Duration;

use crate::REMOTE_PROTOCOL_VERSION;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TailscaleState {
    Missing,
    DaemonStopped,
    SignedOut,
    Unauthorized,
    ServeNotConfigured,
    HealthUnreachable,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TailscaleReport {
    pub state: TailscaleState,
    pub dns_name: Option<String>,
    pub tailnet_ip: Option<String>,
    pub url: Option<String>,
    pub health_verified: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOutput {
    pub found: bool,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl CliOutput {
    fn run(args: &[&str]) -> Self {
        match Command::new("tailscale").args(args).output() {
            Ok(output) => Self {
                found: true,
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self {
                found: false,
                success: false,
                stdout: String::new(),
                stderr: error.to_string(),
            },
            Err(error) => Self {
                found: true,
                success: false,
                stdout: String::new(),
                stderr: error.to_string(),
            },
        }
    }
}

pub fn detect_and_verify(loopback_port: u16) -> TailscaleReport {
    let status = CliOutput::run(&["status", "--json"]);
    let serve = CliOutput::run(&["serve", "status", "--json"]);
    let mut report = classify_outputs(&status, &serve, loopback_port);
    let Some(url) = report.url.clone() else {
        return report;
    };
    match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .and_then(|client| client.get(format!("{url}/healthz")).send())
        .and_then(reqwest::blocking::Response::error_for_status)
        .and_then(reqwest::blocking::Response::json::<Value>)
    {
        Ok(payload)
            if payload.get("status").and_then(Value::as_str) == Some("ok")
                && payload.get("protocolVersion").and_then(Value::as_u64)
                    == Some(u64::from(REMOTE_PROTOCOL_VERSION)) =>
        {
            report.state = TailscaleState::Ready;
            report.health_verified = true;
            report.detail = "Tailscale Serve reaches this host with the expected protocol.".into();
        }
        _ => {
            report.state = TailscaleState::HealthUnreachable;
            report.detail =
                "The Serve route exists, but its /healthz response was not verified.".into();
        }
    }
    report
}

pub fn classify_outputs(
    status: &CliOutput,
    serve: &CliOutput,
    loopback_port: u16,
) -> TailscaleReport {
    if !status.found {
        return report(
            TailscaleState::Missing,
            "The Tailscale CLI is not installed.",
        );
    }
    if !status.success {
        let diagnostic = format!("{} {}", status.stdout, status.stderr).to_ascii_lowercase();
        let state = if diagnostic.contains("not running")
            || diagnostic.contains("failed to connect to local tailscaled")
        {
            TailscaleState::DaemonStopped
        } else if diagnostic.contains("logged out") || diagnostic.contains("not logged in") {
            TailscaleState::SignedOut
        } else {
            TailscaleState::Unauthorized
        };
        return report(state, "Tailscale status is not available to this user.");
    }
    let Ok(status_json) = serde_json::from_str::<Value>(&status.stdout) else {
        return report(
            TailscaleState::Unauthorized,
            "Tailscale returned invalid status data.",
        );
    };
    let backend = status_json
        .get("BackendState")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if backend.eq_ignore_ascii_case("NeedsLogin") {
        return report(TailscaleState::SignedOut, "Tailscale needs a user sign-in.");
    }
    if !backend.eq_ignore_ascii_case("Running") {
        return report(
            TailscaleState::DaemonStopped,
            "The Tailscale daemon is not running.",
        );
    }
    let dns_name = status_json
        .pointer("/Self/DNSName")
        .and_then(Value::as_str)
        .map(|value| value.trim_end_matches('.').to_owned())
        .filter(|value| !value.is_empty());
    let tailnet_ip = status_json
        .get("TailscaleIPs")
        .and_then(Value::as_array)
        .and_then(|values| values.iter().find_map(Value::as_str))
        .map(str::to_owned);
    if !serve.success {
        let diagnostic = format!("{} {}", serve.stdout, serve.stderr).to_ascii_lowercase();
        let state = if diagnostic.contains("permission") || diagnostic.contains("unauthorized") {
            TailscaleState::Unauthorized
        } else {
            TailscaleState::ServeNotConfigured
        };
        let mut result = report(
            state,
            "Tailscale Serve is not configured for Video Creater.",
        );
        result.dns_name = dns_name;
        result.tailnet_ip = tailnet_ip;
        return result;
    }
    let Ok(serve_json) = serde_json::from_str::<Value>(&serve.stdout) else {
        return report(
            TailscaleState::ServeNotConfigured,
            "Tailscale Serve returned invalid status data.",
        );
    };
    let expected_proxy = format!("http://127.0.0.1:{loopback_port}");
    if !value_contains(&serve_json, &expected_proxy) {
        let mut result = report(
            TailscaleState::ServeNotConfigured,
            "Serve does not proxy the expected loopback host port.",
        );
        result.dns_name = dns_name;
        result.tailnet_ip = tailnet_ip;
        return result;
    }
    let Some(dns_name) = dns_name else {
        return report(
            TailscaleState::Unauthorized,
            "MagicDNS name is unavailable.",
        );
    };
    TailscaleReport {
        state: TailscaleState::HealthUnreachable,
        dns_name: Some(dns_name.clone()),
        tailnet_ip,
        url: Some(format!("https://{dns_name}")),
        health_verified: false,
        detail: "Serve route found; /healthz has not been verified yet.".into(),
    }
}

fn value_contains(value: &Value, expected: &str) -> bool {
    match value {
        Value::String(value) => value.trim_end_matches('/') == expected,
        Value::Array(values) => values.iter().any(|value| value_contains(value, expected)),
        Value::Object(values) => values.values().any(|value| value_contains(value, expected)),
        _ => false,
    }
}

fn report(state: TailscaleState, detail: &str) -> TailscaleReport {
    TailscaleReport {
        state,
        dns_name: None,
        tailnet_ip: None,
        url: None,
        health_verified: false,
        detail: detail.into(),
    }
}
