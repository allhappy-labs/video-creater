use video_creater_lib::render_pipeline::codex_e2e::run_codex_e2e_cli;

fn main() {
    match run_codex_e2e_cli(std::env::args()) {
        Ok(result) => {
            let output = serde_json::json!({
                "ok": true,
                "reportPath": result.report_path,
                "codex": result.codex_binary,
                "threadId": result.thread_id,
            });
            println!(
                "{}",
                serde_json::to_string(&output).unwrap_or_else(|_| r#"{"ok":true}"#.to_string())
            );
        }
        Err(errors) => {
            let output = serde_json::to_string(&errors)
                .unwrap_or_else(|_| r#"[{"code":"PIPELINE_INPUT_INVALID"}]"#.to_string());
            eprintln!("{output}");
            std::process::exit(1);
        }
    }
}
