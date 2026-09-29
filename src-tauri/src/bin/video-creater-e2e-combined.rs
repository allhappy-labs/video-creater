use video_creater_lib::render_pipeline::combined_e2e::{parse_combined_e2e_args, run_combined_e2e};
use video_creater_lib::render_runtime::start_render_process_runtime;

fn main() {
    if let Err(error) = start_render_process_runtime() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    match parse_combined_e2e_args(std::env::args()).and_then(|config| run_combined_e2e(&config)) {
        Ok(result) => {
            let output = serde_json::json!({
                "ok": true,
                "finalPath": result.final_path,
                "reportPath": result.json_report_path,
            });
            println!("{}", output);
        }
        Err(errors) => {
            let output = serde_json::to_string(&errors)
                .unwrap_or_else(|_| r#"[{"code":"PIPELINE_INPUT_INVALID"}]"#.to_string());
            eprintln!("{output}");
            std::process::exit(1);
        }
    }
}
