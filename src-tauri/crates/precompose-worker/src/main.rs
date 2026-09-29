use std::process::ExitCode;

fn main() -> ExitCode {
    let exit_code = video_creater_precompose_worker::run_worker(
        std::io::stdin().lock(),
        std::io::stdout().lock(),
    );
    ExitCode::from(u8::try_from(exit_code).unwrap_or(70))
}
