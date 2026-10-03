use super::*;
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn remote_temporal_start_bounds_silent_service_connection() {
    const CHILD_ENV: &str = "VIDEO_CREATER_TEST_SILENT_TEMPORAL_CHILD";
    if std::env::var_os(CHILD_ENV).is_some() {
        let job = temporal_job_summary(
            TemporalWorkflowKind::ExportMedia,
            "isolated-connection-project",
            "isolated-connection-job",
            crate::project::model::JobStatus::Queued,
            "2026-10-03T00:00:00Z",
        );
        let started = Instant::now();
        let error = start_temporal(&job).unwrap_err();
        assert_eq!(
            error.internal_detail(),
            Some("timed out connecting to the workflow service")
        );
        assert!(started.elapsed() < Duration::from_secs(5));
        return;
    }

    // A child process isolates SDK environment configuration from other parallel tests.
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("temporal.toml");
    std::fs::write(&config, "[profile.default]\n").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .env(CHILD_ENV, "1")
        .env("TEMPORAL_CONFIG_FILE", config)
        .env(
            "TEMPORAL_ADDRESS",
            format!("http://{}", listener.local_addr().unwrap()),
        )
        .arg("remote_temporal_start_bounds_silent_service_connection")
        .arg("--nocapture")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Preserve HOME as instructed; the explicit isolated config prevents reading user settings.
    if let Some(home) = std::env::var_os("HOME") {
        command.env("HOME", home);
    }
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut accepted = None;
    let status = loop {
        if accepted.is_none() {
            match listener.accept() {
                Ok((stream, _)) => accepted = Some(stream),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("isolated listener failed: {error}");
                }
            }
        }
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        accepted.is_some(),
        "SDK must connect to the isolated TCP peer"
    );
    assert!(
        status.is_some_and(|status| status.success()),
        "remote Temporal startup must return the shared bounded connection error"
    );
}
