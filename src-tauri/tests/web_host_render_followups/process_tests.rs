//! Child processes isolate native runtime/storage ownership and make crash evidence real.
use super::helpers::*;
use axum::http::StatusCode;
use serde_json::{json, Value};
use std::fs::File;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::export_profiles::ExportProfile;
use video_creater_lib::project::model::{JobStatus, VideoProject};
use video_creater_lib::project::split::load_split_project;
use video_creater_lib::render_pipeline::gstreamer_backend::probe_media_with_gstreamer;
use video_creater_lib::render_pipeline::project_export::{
    MediaRenderAdmission, MediaRenderAttempt, ProjectMediaRenderPaths,
};
use video_creater_lib::web_host::rpc::RpcEnvelope;

const JOB: &str = "actual-encoding-job";
const DATA_ENV: &str = "VIDEO_CREATER_RENDER_FOLLOWUP_TEST_ROOT";
const MODE_ENV: &str = "VIDEO_CREATER_RENDER_FOLLOWUP_TEST_MODE";
const CHILD_LIMIT: Duration = Duration::from_secs(150);

struct Process {
    child: Child,
    log: std::path::PathBuf,
}

impl Process {
    fn launch(root: &Path, mode: &str) -> Self {
        let log = root.join(format!("{mode}.log"));
        let stdout = File::create(&log).unwrap();
        let stderr = stdout.try_clone().unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "process_tests::native_host_process_helper",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(DATA_ENV, root)
            .env(MODE_ENV, mode)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .expect("launch isolated native host process");
        Self { child, log }
    }

    fn wait_success(&mut self) {
        let started = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                let log = std::fs::read_to_string(&self.log).unwrap();
                eprintln!("{log}");
                assert!(status.success(), "native host child failed: {status}");
                return;
            }
            assert!(
                started.elapsed() < CHILD_LIMIT,
                "native host child exceeded {CHILD_LIMIT:?}: {}",
                std::fs::read_to_string(&self.log).unwrap()
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn kill_and_reap(&mut self) {
        self.child.kill().unwrap();
        assert!(!self.child.wait().unwrap().success());
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // A failed assertion must not leave a worker, an OS lock or an encode running.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn actual_encoding_beyond_editor_ttl_preserves_authenticated_liveness() {
    let root = tempfile::tempdir().unwrap();
    Process::launch(root.path(), "long-encode").wait_success();
}

#[test]
fn interrupted_encoding_is_recovered_after_a_real_host_process_restart() {
    let root = tempfile::tempdir().unwrap();
    let mut process = Process::launch(root.path(), "crash-encode");
    let started = Instant::now();
    let checkpoint = root.path().join("encoding-checkpoint.json");
    while !checkpoint.is_file() {
        assert!(
            process.child.try_wait().unwrap().is_none(),
            "crash fixture exited before encoding: {}",
            std::fs::read_to_string(&process.log).unwrap()
        );
        assert!(
            started.elapsed() < Duration::from_secs(90),
            "crash fixture never established progressing encode: {}",
            std::fs::read_to_string(&process.log).unwrap()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let evidence: Value = serde_json::from_slice(&std::fs::read(checkpoint).unwrap()).unwrap();
    assert!(evidence["progress"].as_f64().unwrap() > 0.0);
    assert!(evidence["outputBytes"].as_u64().unwrap() > 0);
    process.kill_and_reap();
    eprintln!(
        "Killed actual encoding child pid={}, evidence={evidence}",
        process.child.id()
    );
    Process::launch(root.path(), "restart-recover").wait_success();
}

// This helper is intentionally ignored by the harness; both parent tests invoke it explicitly.
// Missing runtime, missing CPU affinity or missing environment is a failing prerequisite.
#[test]
#[ignore = "parent tests execute isolated native host processes with explicit data roots"]
fn native_host_process_helper() {
    restrict_native_child_to_one_cpu();
    let root = std::env::var_os(DATA_ENV).expect("parent supplies synthetic data root");
    let root = Path::new(&root);
    let mode = std::env::var(MODE_ENV).expect("parent supplies process mode");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        match mode.as_str() {
            "long-encode" => encode(root, false).await,
            "crash-encode" => encode(root, true).await,
            "restart-recover" => restart(root).await,
            _ => panic!("unknown child mode"),
        }
    });
}

fn restrict_native_child_to_one_cpu() {
    // Constrain the test's codec work, rather than insert artificial pending time or sleeps.
    // All subsequently created Rust/GStreamer threads inherit this allowed CPU.
    unsafe {
        let mut allowed: libc::cpu_set_t = std::mem::zeroed();
        assert_eq!(
            libc::sched_getaffinity(0, std::mem::size_of_val(&allowed), &mut allowed),
            0
        );
        let cpu = (0..libc::CPU_SETSIZE as usize)
            .find(|cpu| libc::CPU_ISSET(*cpu, &allowed))
            .expect("at least one allowed CPU");
        let mut selected: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_SET(cpu, &mut selected);
        assert_eq!(
            libc::sched_setaffinity(0, std::mem::size_of_val(&selected), &selected),
            0
        );
        eprintln!("Native test codec threads restricted to inherited allowed CPU {cpu}");
    }
}

fn long_admission(host: &Host, token: &str) -> RpcEnvelope {
    let mut request = host.admission("long-admission", JOB, token, host.project.content_revision);
    // Real Master VP8 codec work; cancel once the observation contract is met.
    // The large selected range is never required to complete in the long test.
    request.payload["profile"] = json!("webm");
    request.payload["quality"] = json!("final");
    request.payload["encodeTier"] = json!("master");
    request.payload["width"] = json!(1920);
    request.payload["height"] = json!(1080);
    request
}

async fn snapshot(host: &Host, browser: &Browser, id: &str) -> VideoProject {
    decode(
        host.success(
            browser,
            &host.envelope(
                id,
                "read_project_snapshot_from_split_project_folder",
                None,
                None,
                json!({}),
            ),
        )
        .await
        .unwrap(),
    )
    .unwrap()
}

async fn progress(host: &Host, browser: &Browser, cycle: u32) -> f64 {
    let value = host
        .success(
            browser,
            &host.envelope(
                &format!("encode-progress-{cycle}"),
                "load_job_progress_from_split_project_folder",
                None,
                None,
                json!({}),
            ),
        )
        .await
        .unwrap();
    value
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["jobId"] == JOB)
        .and_then(|value| value["progress"].as_f64())
        .unwrap_or(0.0)
}

fn output_bytes(host: &Host) -> u64 {
    let paths = ProjectMediaRenderPaths::new(JOB, ExportProfile::Webm).unwrap();
    std::fs::metadata(host.path.join(paths.output_path))
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

async fn encode(root: &Path, crash: bool) {
    let host = Host::create(root, "process-encode", 600.0);
    let first = host.pair("Encoding editor").await.unwrap();
    let second = host.pair("Replacement editor").await.unwrap();
    let lease = host.lease(&first, false).await.unwrap();
    let token = lease["editorLeaseToken"].as_str().unwrap();
    let request = long_admission(&host, token);
    let admission: MediaRenderAdmission =
        decode(host.success(&first, &request).await.unwrap()).unwrap();
    let mut persisted_input = request.clone();
    persisted_input.editor_lease_token = None;
    std::fs::write(
        root.join("original-admission.json"),
        serde_json::to_vec(&persisted_input).unwrap(),
    )
    .unwrap();
    let observation_deadline = Instant::now() + Duration::from_secs(100);
    let mut began = None;
    let mut first_progress = 0.0;
    let mut first_bytes = 0;
    let mut previous_progress = 0.0;
    let mut last_advance = None;
    let mut advances = 0;
    let mut cycle = 0;
    loop {
        assert!(
            Instant::now() < observation_deadline,
            "encode did not advance within its bounded observation window"
        );
        assert_eq!(
            host.attempt(&first, JOB, cycle).await.unwrap(),
            MediaRenderAttempt::Pending,
            "large real encode ended before the observation contract"
        );
        let p = progress(&host, &first, cycle).await;
        assert!(p >= previous_progress, "native encode progress decreased");
        let bytes = output_bytes(&host);
        if p > 0.0 && bytes > 0 && began.is_none() {
            began = Some(Instant::now());
            first_progress = p;
            first_bytes = bytes;
        }
        if p > previous_progress {
            advances += 1;
            last_advance = Some(Instant::now());
        }
        previous_progress = p;
        if began.is_some() {
            assert!(
                last_advance.is_some_and(|at| at.elapsed() < Duration::from_secs(10)),
                "encoding stopped advancing during the TTL observation"
            );
        }
        let renewed = host.lease(&first, false).await.unwrap();
        assert_eq!(renewed["editorLeaseToken"], lease["editorLeaseToken"]);
        let current = snapshot(&host, &first, &format!("active-{cycle}")).await;
        assert!(current.jobs.iter().any(|job| {
            job.id == JOB
                && (job.status == JobStatus::Running
                    || (began.is_none() && job.status == JobStatus::Queued))
        }));
        let mut other = host.envelope(
            &format!("other-{cycle}"),
            "read_project_snapshot_from_split_project_folder",
            None,
            None,
            json!({}),
        );
        other.project_id = Some(host.other_id.clone());
        let other: VideoProject = decode(host.success(&first, &other).await.unwrap()).unwrap();
        assert!(other.jobs.is_empty());
        if crash
            && began.is_some_and(|start| start.elapsed() > Duration::from_secs(2))
            && p > first_progress
            && bytes > first_bytes
        {
            // Publish only after native encode position and output both advanced.
            let checkpoint = json!({"progress": p, "outputBytes": bytes, "advances": advances});
            std::fs::write(
                root.join("encoding-checkpoint.tmp"),
                serde_json::to_vec(&checkpoint).unwrap(),
            )
            .unwrap();
            std::fs::rename(
                root.join("encoding-checkpoint.tmp"),
                root.join("encoding-checkpoint.json"),
            )
            .unwrap();
            // The parent interrupts this process while the actual admitted worker runs.
            std::future::pending::<()>().await;
        }
        if !crash && began.is_some_and(|start| start.elapsed() > Duration::from_secs(31)) {
            assert!(
                p > first_progress
                    && p < 0.95
                    && bytes > first_bytes
                    && advances >= 3
                    && last_advance.is_some_and(|at| at.elapsed() < Duration::from_secs(3)),
                "actual encoding must advance for the full TTL interval"
            );
            let elapsed = began.unwrap().elapsed();
            eprintln!("Actual advancing native encode observed for {elapsed:?}: position {first_progress:.6}->{p:.6}, output {first_bytes}->{bytes} bytes, {advances} progress advances");
            break;
        }
        cycle += 1;
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    assert!(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            > lease["expiresAt"].as_u64().unwrap()
    );
    let current = snapshot(&host, &first, "edit-current").await;
    let edit = host.envelope("edit-during-encode", "apply_project_action_to_split_project_folder", Some(token), Some(current.content_revision), json!({"action": ProjectAction::UpdateProjectSettings { name: "Committed during actual encoding".into(), render_settings: current.render_settings.clone() }}));
    host.success(&first, &edit).await.unwrap();
    let replay: MediaRenderAdmission =
        decode(host.success(&first, &request).await.unwrap()).unwrap();
    assert_eq!(replay, admission);
    let live: MediaRenderAttempt = decode(
        host.success(
            &first,
            &host.envelope(
                "recover-live-encode",
                "recover_render_attempt_in_split_project_folder",
                Some(token),
                None,
                json!({"jobId": JOB, "attemptId": admission.attempt_id}),
            ),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert_eq!(live, MediaRenderAttempt::Pending);
    let replacement = host.lease(&second, true).await.unwrap();
    let replacement_token = replacement["editorLeaseToken"].as_str().unwrap();
    let current = snapshot(&host, &second, "takeover-current").await;
    let mut cancel = host.envelope(
        "stale-cancel",
        "cancel_render_job_in_split_project_folder",
        Some(token),
        Some(current.content_revision),
        json!({"jobId": JOB, "attemptId": admission.attempt_id, "updatedAt": UPDATED_AT}),
    );
    assert_eq!(
        host.rpc(&first, &cancel).await.unwrap().0,
        StatusCode::CONFLICT
    );
    cancel.request_id = "replacement-cancel".into();
    cancel.editor_lease_token = Some(replacement_token.into());
    host.success(&second, &cancel).await.unwrap();
    assert!(matches!(
        host.terminal(&second, JOB).await.unwrap(),
        MediaRenderAttempt::Failed {
            interrupted: false,
            ..
        }
    ));
    let final_project = snapshot(&host, &second, "cancelled-final").await;
    assert_eq!(final_project.name, "Committed during actual encoding");
    assert_eq!(
        final_project
            .jobs
            .iter()
            .find(|job| job.id == JOB)
            .unwrap()
            .status,
        JobStatus::Cancelled
    );
    assert!(final_project.render_reports.is_empty());
}

async fn restart(root: &Path) {
    let path = root.join("projects/process-encode.palmier");
    let before = load_split_project(&path).unwrap();
    let host = Host::open(root, &path);
    let browser = host.pair("Editor after host restart").await.unwrap();
    let lease = host.lease(&browser, false).await.unwrap();
    let token = lease["editorLeaseToken"].as_str().unwrap();
    let original: RpcEnvelope =
        serde_json::from_slice(&std::fs::read(root.join("original-admission.json")).unwrap())
            .unwrap();
    let interrupted = host.attempt(&browser, JOB, 0).await.unwrap();
    assert!(matches!(
        interrupted,
        MediaRenderAttempt::Failed {
            interrupted: true,
            ..
        }
    ));
    assert_eq!(
        load_split_project(&path).unwrap(),
        before,
        "status polling after restart must not mutate canonical state"
    );
    let mut replay = original.clone();
    replay.request_id = "replay-after-process-restart".into();
    replay.editor_lease_token = Some(token.into());
    let acknowledgement: MediaRenderAdmission =
        decode(host.success(&browser, &replay).await.unwrap()).unwrap();
    assert_eq!(
        acknowledgement.source_revision,
        original.expected_revision.unwrap()
    );
    assert_eq!(
        host.attempt(&browser, JOB, 1).await.unwrap(),
        interrupted,
        "durable admission replay must not restart encoding"
    );
    let bytes_before = output_bytes(&host);
    let recover = host.envelope(
        "recover-interrupted",
        "recover_render_attempt_in_split_project_folder",
        Some(token),
        None,
        json!({"jobId": JOB, "attemptId": acknowledgement.attempt_id}),
    );
    let wrong = Browser {
        csrf: "invalid".into(),
        ..browser.clone()
    };
    assert_eq!(
        host.request(
            "/api/v1/rpc",
            Some(&wrong),
            serde_json::to_value(&recover).unwrap(),
        )
        .await
        .unwrap()
        .0,
        StatusCode::FORBIDDEN
    );
    let recovered: MediaRenderAttempt =
        decode(host.success(&browser, &recover).await.unwrap()).unwrap();
    assert!(matches!(
        recovered,
        MediaRenderAttempt::Failed {
            interrupted: true,
            ..
        }
    ));
    let settled = snapshot(&host, &browser, "recovered-current").await;
    assert_eq!(
        settled
            .jobs
            .iter()
            .find(|job| job.id == JOB)
            .unwrap()
            .status,
        JobStatus::Failed
    );
    assert_eq!(
        settled.content_revision, before.content_revision,
        "recovery is bookkeeping"
    );
    assert!(settled.render_reports.is_empty());
    assert_eq!(
        output_bytes(&host),
        bytes_before,
        "recovery must not publish or restart partial output"
    );
    let duplicate: MediaRenderAttempt = decode(
        host.success(
            &browser,
            &host.envelope(
                "recover-interrupted-again",
                &recover.operation,
                Some(token),
                None,
                recover.payload.clone(),
            ),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(duplicate, MediaRenderAttempt::Failed { .. }));
    let mut retry = host.admission(
        "explicit-new-attempt",
        "restart-short-job",
        token,
        settled.content_revision,
    );
    retry.payload["rangeStartSeconds"] = json!(0.0);
    retry.payload["rangeEndSeconds"] = json!(2.0);
    host.success(&browser, &retry).await.unwrap();
    let MediaRenderAttempt::Completed { result } =
        host.terminal(&browser, "restart-short-job").await.unwrap()
    else {
        panic!("explicit retry must complete after released process locks");
    };
    let output = host.path.join(&result.output_path);
    let (probe, _) =
        probe_media_with_gstreamer(&output, Duration::from_secs(30), "restartActualNative")
            .unwrap();
    assert!(probe.video.is_some() && probe.audio.is_some());
    assert!((probe.duration_seconds.unwrap() - 2.0).abs() < 0.5);
    let latest = load_split_project(&path).unwrap();
    assert_eq!(latest.jobs.iter().filter(|job| job.id == JOB).count(), 1);
    assert_eq!(
        latest.jobs.iter().find(|job| job.id == JOB).unwrap().status,
        JobStatus::Failed
    );
    assert!(!latest.render_reports.iter().any(|report| report.id == JOB));
    assert_eq!(
        latest
            .jobs
            .iter()
            .find(|job| job.id == "restart-short-job")
            .unwrap()
            .status,
        JobStatus::Completed
    );
    eprintln!("Fresh host process recovered interrupted encode; explicit new native retry stream-checked at {}", output.display());
}
