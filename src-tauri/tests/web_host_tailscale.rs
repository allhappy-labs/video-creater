use video_creater_lib::web_host::tailscale::{classify_outputs, CliOutput, TailscaleState};

fn output(success: bool, stdout: &str, stderr: &str) -> CliOutput {
    CliOutput {
        found: true,
        success,
        stdout: stdout.into(),
        stderr: stderr.into(),
    }
}

#[test]
fn reports_missing_stopped_signed_out_and_unauthorized_separately() {
    let missing = CliOutput {
        found: false,
        success: false,
        stdout: String::new(),
        stderr: String::new(),
    };
    assert_eq!(
        classify_outputs(&missing, &output(false, "", ""), 4777).state,
        TailscaleState::Missing
    );
    assert_eq!(
        classify_outputs(
            &output(
                false,
                "",
                "failed to connect to local tailscaled; it doesn't appear to be running"
            ),
            &output(false, "", ""),
            4777,
        )
        .state,
        TailscaleState::DaemonStopped
    );
    assert_eq!(
        classify_outputs(
            &output(true, r#"{"BackendState":"NeedsLogin"}"#, ""),
            &output(false, "", ""),
            4777,
        )
        .state,
        TailscaleState::SignedOut
    );
    assert_eq!(
        classify_outputs(
            &output(false, "", "permission denied"),
            &output(false, "", ""),
            4777,
        )
        .state,
        TailscaleState::Unauthorized
    );
}

#[test]
fn exposes_no_url_until_magic_dns_and_exact_serve_proxy_are_verified() {
    let status = output(
        true,
        r#"{"BackendState":"Running","Self":{"DNSName":"host.tail.example."},"TailscaleIPs":["100.64.0.4"]}"#,
        "",
    );
    let absent = classify_outputs(&status, &output(true, r#"{"Web":{}}"#, ""), 4777);
    assert_eq!(absent.state, TailscaleState::ServeNotConfigured);
    assert!(absent.url.is_none());

    let routed = classify_outputs(
        &status,
        &output(
            true,
            r#"{"Web":{"host.tail.example:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:4777"}}}}}"#,
            "",
        ),
        4777,
    );
    assert_eq!(routed.state, TailscaleState::HealthUnreachable);
    assert_eq!(routed.url.as_deref(), Some("https://host.tail.example"));
    assert_eq!(routed.tailnet_ip.as_deref(), Some("100.64.0.4"));
    assert!(!routed.health_verified);
}
