use std::path::PathBuf;

use video_creater_lib::media_inspection::{inspect_image, inspect_render_directory, inspect_video};

fn main() {
    match run() {
        Ok(output) => println!("{output}"),
        Err(error) => {
            let output = serde_json::json!({
                "ok": false,
                "error": error.to_string(),
            });
            eprintln!("{output}");
            std::process::exit(1);
        }
    }
}

fn run() -> anyhow::Result<String> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        anyhow::bail!("Usage: video-creater-inspect <image|video|render-dir> <path>");
    };
    let Some(path) = args.next() else {
        anyhow::bail!("Missing path for {command}");
    };
    if args.next().is_some() {
        anyhow::bail!("Too many arguments");
    }

    let path = PathBuf::from(path);
    match command.as_str() {
        "image" => Ok(serde_json::to_string_pretty(&inspect_image(&path)?)?),
        "video" => Ok(serde_json::to_string_pretty(&inspect_video(&path)?)?),
        "render-dir" => Ok(serde_json::to_string_pretty(&inspect_render_directory(
            &path,
        )?)?),
        _ => anyhow::bail!("Unknown inspection command '{command}'"),
    }
}
