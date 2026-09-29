//! `video-creater-speech`: Linux on-device transcription and speech-analysis helper.
//!
//! Implements the same stdin/stdout JSON contract as the macOS FluidAudio helper using
//! sherpa-onnx (Parakeet TDT 0.6B v3, pyannote segmentation 3.0, WeSpeaker ResNet34-LM) and the
//! upstream Silero VAD ONNX model on ONNX Runtime.
//!
//! Without the `native` feature only the protocol layer is compiled (for tests and workspace
//! checks) and every request fails with a runtime-unavailable error.

#![cfg_attr(not(all(feature = "native", target_os = "linux")), allow(dead_code))]

mod chunking;
mod models;
mod protocol;
mod speakers;
mod vad;
mod words;

#[cfg(all(feature = "native", target_os = "linux"))]
mod audio;
#[cfg(all(feature = "native", target_os = "linux"))]
mod engine;
#[cfg(all(feature = "native", target_os = "linux"))]
mod sherpa;

use std::io::{Read, Write};
use std::process::ExitCode;

use protocol::{decode_helper_request, error_response, HelperError, HelperRequest};

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("{}", version_line());
        return ExitCode::SUCCESS;
    }
    let mut input = Vec::new();
    if let Err(error) = std::io::stdin().read_to_end(&mut input) {
        return fail(&HelperError::InvalidRequest(error.to_string()));
    }
    let result = decode_helper_request(&input).and_then(|request| run(&request));
    match result {
        Ok(json) => {
            let mut stdout = std::io::stdout().lock();
            if writeln!(stdout, "{json}")
                .and_then(|()| stdout.flush())
                .is_err()
            {
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(error) => fail(&error),
    }
}

fn fail(error: &HelperError) -> ExitCode {
    if let Ok(json) = serde_json::to_string(&error_response(error)) {
        let mut stdout = std::io::stdout().lock();
        let _ = writeln!(stdout, "{json}");
        let _ = stdout.flush();
    }
    ExitCode::FAILURE
}

fn serialize(value: &impl serde::Serialize) -> Result<String, HelperError> {
    serde_json::to_string(value).map_err(|error| HelperError::Runtime(error.to_string()))
}

#[cfg(all(feature = "native", target_os = "linux"))]
fn run(request: &HelperRequest) -> Result<String, HelperError> {
    match request {
        HelperRequest::Transcription(request) => serialize(&engine::transcribe(request)?),
        HelperRequest::SpeechAnalysis(request) => serialize(&engine::analyze_speech(request)?),
    }
}

#[cfg(not(all(feature = "native", target_os = "linux")))]
fn run(request: &HelperRequest) -> Result<String, HelperError> {
    let _ = request;
    Err(HelperError::NativeRuntimeUnavailable)
}

#[cfg(all(feature = "native", target_os = "linux"))]
fn version_line() -> String {
    format!(
        "video-creater-speech {} ({})",
        env!("CARGO_PKG_VERSION"),
        sherpa::runtime_versions()
    )
}

#[cfg(not(all(feature = "native", target_os = "linux")))]
fn version_line() -> String {
    format!(
        "video-creater-speech {} (native runtime disabled)",
        env!("CARGO_PKG_VERSION")
    )
}
