use std::io::{self, BufReader};
use std::path::PathBuf;
use video_creater_lib::codex::mcp_server::run_mcp_stdio;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let project_dir = parse_project_dir(std::env::args().skip(1))?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_mcp_stdio(&project_dir, BufReader::new(stdin.lock()), stdout.lock())
}

fn parse_project_dir<I>(mut args: I) -> Result<PathBuf, String>
where
    I: Iterator<Item = String>,
{
    while let Some(arg) = args.next() {
        if arg == "--project-dir" {
            let value = args
                .next()
                .ok_or_else(|| "--project-dir requires a value".to_string())?;
            let path = PathBuf::from(value);
            if !path.is_absolute() {
                return Err("--project-dir must be an absolute path".to_string());
            }
            return Ok(path);
        }
        if arg == "--help" || arg == "-h" {
            return Err(
                "usage: video-creater-mcp-server --project-dir /absolute/project".to_string(),
            );
        }
    }

    Err("usage: video-creater-mcp-server --project-dir /absolute/project".to_string())
}
