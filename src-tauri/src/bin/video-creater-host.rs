use video_creater_lib::codex::app_server::{
    bundled_codex_app_server_command, initialize_codex_app_server, CodexAppServerTransport,
    StdioCodexAppServerTransport,
};
use video_creater_lib::web_host::config::HostConfig;

#[tokio::main]
async fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.iter().any(|argument| argument == "--tailscale-status") {
        let report = video_creater_lib::web_host::tailscale::detect_and_verify(
            video_creater_lib::web_host::config::DEFAULT_HOST_PORT,
        );
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("Tailscale report serializes")
        );
        return;
    }
    if args
        .iter()
        .any(|argument| argument == "--codex-sidecar-smoke")
    {
        if let Err(error) = smoke_codex_sidecar() {
            eprintln!("video-creater-host: {error}");
            std::process::exit(1);
        }
        return;
    }
    let config = HostConfig::from_args(args).unwrap_or_else(|error| {
        eprintln!("video-creater-host: {error}");
        std::process::exit(2);
    });
    if let Some(warning) = config.insecure_bind_warning() {
        eprintln!("{warning}");
    }
    if let Err(error) = video_creater_lib::render_runtime::start_render_process_runtime() {
        eprintln!("video-creater-host: required render runtime could not start: {error}");
        std::process::exit(1);
    }
    if let Err(error) = video_creater_lib::web_host::run(config).await {
        eprintln!("video-creater-host: {error}");
        std::process::exit(1);
    }
}

fn smoke_codex_sidecar() -> Result<(), String> {
    let smoke_home = tempfile::tempdir().map_err(|error| {
        format!("temporary Codex smoke-test home could not be created: {error}")
    })?;
    let original_codex_home = std::env::var_os("CODEX_HOME");
    std::env::set_var("CODEX_HOME", smoke_home.path());
    let result = smoke_codex_sidecar_with_isolated_home();
    match original_codex_home {
        Some(value) => std::env::set_var("CODEX_HOME", value),
        None => std::env::remove_var("CODEX_HOME"),
    }
    result
}

fn smoke_codex_sidecar_with_isolated_home() -> Result<(), String> {
    let command = bundled_codex_app_server_command().map_err(|error| error.to_string())?;
    let mut transport = StdioCodexAppServerTransport::spawn(&command)
        .map_err(|error| format!("bundled Codex app-server did not launch: {error}"))?;
    let initialized = initialize_codex_app_server(&mut transport, 1);
    let cleanup = transport
        .terminate()
        .map_err(|error| format!("bundled Codex app-server cleanup failed: {error}"))?;
    if let Err(error) = initialized {
        let diagnostic = cleanup.stderr.trim();
        return Err(if diagnostic.is_empty() {
            format!("bundled Codex app-server did not initialize: {error}")
        } else {
            format!("bundled Codex app-server did not initialize: {error}; {diagnostic}")
        });
    }
    if !cleanup.reaped {
        return Err("bundled Codex app-server process was not reaped".into());
    }
    println!(r#"{{"status":"passed","codexAppServerInitialized":true}}"#);
    Ok(())
}
