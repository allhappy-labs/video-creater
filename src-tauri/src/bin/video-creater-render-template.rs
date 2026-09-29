use video_creater_lib::render_pipeline::template_project::{
    parse_render_template_args, run_render_template_project,
};
use video_creater_lib::render_runtime::start_render_process_runtime;

fn main() {
    if let Err(error) = start_render_process_runtime() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    match parse_render_template_args(std::env::args())
        .and_then(|config| run_render_template_project(&config))
    {
        Ok(result) => {
            let output = serde_json::json!({
                "ok": true,
                "videoPath": result.video_path,
                "reportPath": result.report_path,
                "graphicsDir": result.graphics_dir,
                "previewPath": result.preview_path,
                "frameCount": result.frame_count,
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
