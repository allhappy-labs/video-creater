//! Isolated visual-precompose worker process plumbing.
//!
//! Native rendering and expression evaluation are contained in this process so
//! a malformed animation cannot terminate the editor process.

mod renderer;

use std::io::{Read, Write};
use video_creater_precompose_protocol::{
    PrecomposeRequest, WorkerError, WorkerErrorCode, WorkerEvent, WorkerPhase,
    PRECOMPOSE_PROTOCOL_NAME, PRECOMPOSE_PROTOCOL_VERSION,
};

pub const EXIT_OK: i32 = 0;
pub const EXIT_BAKE_FAILED: i32 = 1;
pub const EXIT_INVALID_REQUEST: i32 = 2;
pub const EXIT_INTERNAL: i32 = 70;

pub fn run_worker(mut input: impl Read, mut output: impl Write) -> i32 {
    let request = match serde_json::from_reader::<_, PrecomposeRequest>(&mut input) {
        Ok(request) => request,
        Err(error) => {
            let event = WorkerEvent::failed(
                None,
                WorkerError {
                    code: WorkerErrorCode::InvalidRequest,
                    message: format!("request JSON is invalid: {error}"),
                    field: None,
                    retryable: false,
                },
            );
            return write_final_event(&mut output, &event, EXIT_INVALID_REQUEST);
        }
    };

    if let Err(error) = request.validate() {
        let event = WorkerEvent::failed(
            Some(request.request_id),
            WorkerError::invalid_request(error),
        );
        return write_final_event(&mut output, &event, EXIT_INVALID_REQUEST);
    }

    if write_event(
        &mut output,
        &WorkerEvent::progress(
            &request.request_id,
            WorkerPhase::Accepted,
            0,
            request.render.frame_count,
            "request validated; expression evaluation is required",
        ),
    )
    .is_err()
    {
        return EXIT_INTERNAL;
    }

    let result = renderer::bake_lottie(&request, |phase, completed, total, message| {
        write_event(
            &mut output,
            &WorkerEvent::progress(&request.request_id, phase, completed, total, message),
        )
        .map_err(|error| renderer::BakeFailure {
            code: WorkerErrorCode::Io,
            message: format!("unable to write worker progress event: {error}"),
            field: None,
        })
    });
    match result {
        Ok(result) => write_final_event(
            &mut output,
            &WorkerEvent::Completed {
                protocol: PRECOMPOSE_PROTOCOL_NAME.to_string(),
                schema_version: PRECOMPOSE_PROTOCOL_VERSION,
                request_id: Some(request.request_id),
                result,
            },
            EXIT_OK,
        ),
        Err(error) => write_final_event(
            &mut output,
            &WorkerEvent::failed(Some(request.request_id), error.into_worker_error()),
            EXIT_BAKE_FAILED,
        ),
    }
}

fn write_final_event(output: &mut impl Write, event: &WorkerEvent, success_code: i32) -> i32 {
    if write_event(output, event).is_err() {
        EXIT_INTERNAL
    } else {
        success_code
    }
}

fn write_event(output: &mut impl Write, event: &WorkerEvent) -> std::io::Result<()> {
    serde_json::to_writer(&mut *output, event).map_err(std::io::Error::other)?;
    output.write_all(b"\n")?;
    output.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request_json(expressions_enabled: bool) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "protocol": "video-creater.precompose",
            "schemaVersion": PRECOMPOSE_PROTOCOL_VERSION,
            "requestId": "request-1",
            "cacheKey": "a".repeat(64),
            "operation": "bakeLottieRgba",
            "source": {
                "path": "/project/media/title.json",
                "sha256": "b".repeat(64),
                "format": "lottieJson",
                "animationId": null
            },
            "render": {
                "width": 320,
                "height": 180,
                "fps": { "numerator": 30, "denominator": 1 },
                "firstFrame": 0,
                "frameCount": 2,
                "sourceStartMicros": 0,
                "playbackRateMicros": 1000000,
                "looping": true,
                "alphaMode": "straight",
                "colorSpace": "srgb"
            },
            "inputs": { "themeId": null, "slots": [], "marker": null, "segment": null, "stateMachine": null },
            "expressions": { "enabled": expressions_enabled },
            "budgets": {
                "maxFrames": 2,
                "maxPixelsPerFrame": 57600,
                "maxSourceBytes": 1048576,
                "maxArchiveEntries": 64,
                "maxExpandedArchiveBytes": 4194304,
                "maxCompressionRatio": 100,
                "maxWallTimeMs": 1000,
                "maxMemoryBytes": 67108864,
                "maxOutputBytes": 1048576
            },
            "output": { "stagingDir": "/project/cache/.staging/request-1" }
        }))
        .expect("serialize request")
    }

    fn parse_events(output: &[u8]) -> Vec<WorkerEvent> {
        std::str::from_utf8(output)
            .expect("utf8 output")
            .lines()
            .map(|line| serde_json::from_str(line).expect("valid worker event"))
            .collect()
    }

    #[test]
    fn valid_request_with_missing_source_emits_progress_and_one_explicit_failure() {
        let mut output = Vec::new();
        let exit_code = run_worker(request_json(true).as_slice(), &mut output);
        let events = parse_events(&output);

        assert_eq!(exit_code, EXIT_BAKE_FAILED);
        assert_eq!(events.len(), 2);
        assert!(!events[0].is_final());
        assert!(events[1].is_final());
        assert!(matches!(
            &events[1],
            WorkerEvent::Failed { error, .. }
                if error.code == WorkerErrorCode::SourceInvalid
        ));
    }

    #[test]
    fn disabled_expressions_fail_before_any_progress_event() {
        let mut output = Vec::new();
        let exit_code = run_worker(request_json(false).as_slice(), &mut output);
        let events = parse_events(&output);

        assert_eq!(exit_code, EXIT_INVALID_REQUEST);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            WorkerEvent::Failed { error, .. }
                if error.code == WorkerErrorCode::InvalidRequest
                    && error.field.as_deref() == Some("expressions.enabled")
        ));
    }

    #[test]
    fn malformed_json_emits_machine_readable_final_failure() {
        let mut output = Vec::new();
        let exit_code = run_worker(b"not-json".as_slice(), &mut output);
        let events = parse_events(&output);

        assert_eq!(exit_code, EXIT_INVALID_REQUEST);
        assert_eq!(events.len(), 1);
        assert!(events[0].is_final());
        assert!(matches!(
            &events[0],
            WorkerEvent::Failed {
                request_id: None,
                error,
                ..
            } if error.code == WorkerErrorCode::InvalidRequest
        ));
    }
}
