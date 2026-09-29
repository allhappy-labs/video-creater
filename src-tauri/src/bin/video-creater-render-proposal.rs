use video_creater_lib::render_pipeline::proposal::{
    parse_render_proposal_args, run_render_proposal,
};
use video_creater_lib::render_runtime::start_render_process_runtime;

fn main() {
    if let Err(error) = start_render_process_runtime() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    match parse_render_proposal_args(std::env::args())
        .and_then(|config| run_render_proposal(&config))
    {
        Ok(result) => {
            let output = serde_json::json!({
                "ok": true,
                "finalPath": result.final_path,
                "reportPath": result.render_report_path,
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
