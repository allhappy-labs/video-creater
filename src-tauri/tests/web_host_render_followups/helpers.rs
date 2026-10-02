use axum::body::{to_bytes, Body};
use axum::extract::ConnectInfo;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tower::ServiceExt;
use video_creater_lib::project::export_profiles::{
    mp4_export_profile_availability_report, ExportProfile,
};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::VideoProject;
use video_creater_lib::project::split::{load_split_project, save_split_project};
use video_creater_lib::render_pipeline::gstreamer_backend::generate_fixture_source_with_gstreamer;
use video_creater_lib::render_pipeline::project_export::{
    read_media_render_attempt, MediaRenderAttempt,
};
use video_creater_lib::render_runtime::start_render_process_runtime;
use video_creater_lib::web_host::dispatcher::HostDispatcher;
use video_creater_lib::web_host::http::{host_router, HostHttpConfig};
use video_creater_lib::web_host::project_catalog::ProjectCatalog;
use video_creater_lib::web_host::rpc::{RpcEnvelope, RpcResponse};

pub(super) const ORIGIN: &str = "http://host.test";
pub(super) const UPDATED_AT: &str = "2026-10-01T12:00:00Z";
pub(super) type Check<T> = Result<T, String>;

pub(super) fn require(condition: bool, message: &str) -> Check<()> {
    condition.then_some(()).ok_or_else(|| message.into())
}

pub(super) fn decode<T: DeserializeOwned>(value: Value) -> Check<T> {
    serde_json::from_value(value).map_err(|error| error.to_string())
}

#[derive(Clone)]
pub(super) struct Browser {
    pub(super) cookie: String,
    pub(super) csrf: String,
}

pub(super) struct Host {
    _root: Option<tempfile::TempDir>,
    pub(super) app: Router,
    pub(super) path: PathBuf,
    pub(super) project: VideoProject,
    pub(super) project_id: String,
    pub(super) other_id: String,
}

impl Host {
    pub(super) fn new(label: &str) -> Self {
        start_render_process_runtime().expect("BLOCKED: prepared Linux render runtime is required");
        let availability = mp4_export_profile_availability_report()
            .into_iter()
            .find(|profile| profile.profile == ExportProfile::Mp4H264)
            .unwrap();
        assert!(
            availability.available,
            "BLOCKED: prepared Linux MP4 encoder unavailable: {:?}",
            availability.unavailable_reason
        );
        let root = tempfile::tempdir().unwrap();
        let mut host = Self::create(root.path(), label, 6.0);
        host._root = Some(root);
        host
    }

    pub(super) fn create(root: &std::path::Path, label: &str, duration: f64) -> Self {
        start_render_process_runtime().expect("BLOCKED: prepared Linux render runtime is required");
        let projects = root.join("projects");
        let path = projects.join(format!("{label}.palmier"));
        std::fs::create_dir_all(path.join("media")).unwrap();
        generate_fixture_source_with_gstreamer(
            &path.join("media/input.mp4"),
            320,
            180,
            24.0,
            duration + 1.0,
            Duration::from_secs(60),
        )
        .expect("BLOCKED: synthetic MP4 fixture generation failed");
        let mut project = sample_project();
        project.id = label.into();
        project.render_settings.width = 320;
        project.render_settings.height = 180;
        project.render_settings.fps = 24.0;
        project.timeline.duration_seconds = duration;
        project.media[0].duration_seconds = duration + 1.0;
        project.media[0].relative_path = "media/input.mp4".into();
        project.media[0].width = Some(320);
        project.media[0].height = Some(180);
        project.media[0].fps = Some(24.0);
        project.timeline.tracks[0].items[0].duration_seconds = duration;
        project.timeline.tracks[0].items[0]
            .properties
            .insert("sourceIn".into(), json!(0.0));
        project.timeline.tracks[0].items[0]
            .properties
            .insert("sourceOut".into(), json!(duration));
        save_split_project(&path, &project).unwrap();
        let other_path = projects.join("other.palmier");
        let mut other = sample_project();
        other.id = format!("{label}-other");
        save_split_project(&other_path, &other).unwrap();
        Self::open(root, &path)
    }

    pub(super) fn open(root: &std::path::Path, path: &std::path::Path) -> Self {
        start_render_process_runtime().expect("BLOCKED: prepared Linux render runtime is required");
        let project = load_split_project(path).unwrap();
        let projects = root.join("projects");
        let other_path = projects.join("other.palmier");
        let catalog = ProjectCatalog::new(vec![projects]).unwrap();
        let project_id = catalog.id_for_path(path).unwrap();
        let other_id = catalog.id_for_path(&other_path).unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let app = host_router(
            HostHttpConfig {
                assets_dir: root.to_path_buf(),
                session_file: root.join("sessions.json"),
                public_origin: ORIGIN.into(),
                host_label: "Synthetic render host".into(),
                pairing_code: "123456".into(),
                reusable_pairing_code: true,
                issued_at: now,
                now_override: None,
                uploads_dir: root.join("uploads"),
                upload_max_bytes: 1024,
                upload_reserve_bytes: 0,
                project_catalog: catalog.clone(),
                // Exercise trusted proxy identity handling rather than the direct-test bypass.
                // ConnectInfo and headers below simulate Serve; this does not verify live Serve.
                allow_direct_test_identity: false,
            },
            Arc::new(HostDispatcher::with_project_catalog(catalog)),
        )
        .unwrap();
        Self {
            _root: None,
            app,
            path: path.to_path_buf(),
            project,
            project_id,
            other_id,
        }
    }

    pub(super) fn envelope(
        &self,
        id: &str,
        operation: &str,
        token: Option<&str>,
        revision: Option<u64>,
        payload: Value,
    ) -> RpcEnvelope {
        RpcEnvelope {
            request_id: id.into(),
            operation: operation.into(),
            project_id: Some(self.project_id.clone()),
            expected_revision: revision,
            editor_lease_token: token.map(str::to_owned),
            payload,
        }
    }

    pub(super) fn admission(&self, id: &str, job: &str, token: &str, revision: u64) -> RpcEnvelope {
        self.envelope(
            id,
            "render_media_to_split_project_folder",
            Some(token),
            Some(revision),
            json!({
                "admissionProtocol": 1,
                "projectId": self.project.id,
                "profile": "mp4H264", "quality": "draft", "width": 320, "height": 180,
                "jobId": job, "attemptId": format!("render-attempt/{job}"), "updatedAt": UPDATED_AT
            }),
        )
    }

    pub(super) async fn request(
        &self,
        path: &str,
        browser: Option<&Browser>,
        value: Value,
    ) -> Check<(StatusCode, Value, Option<String>)> {
        let mut builder = Request::post(path)
            .header(header::ORIGIN, ORIGIN)
            .header(header::CONTENT_TYPE, "application/json")
            .header("tailscale-user-login", "synthetic@example.test")
            .header("tailscale-user-name", "Synthetic test");
        if let Some(browser) = browser {
            builder = builder
                .header(header::COOKIE, &browser.cookie)
                .header("x-csrf-token", &browser.csrf);
        }
        let mut request = builder
            .body(Body::from(
                serde_json::to_vec(&value).map_err(|e| e.to_string())?,
            ))
            .map_err(|e| e.to_string())?;
        request.extensions_mut().insert(ConnectInfo(
            "127.0.0.1:12345".parse::<SocketAddr>().unwrap(),
        ));
        let response =
            tokio::time::timeout(Duration::from_secs(5), self.app.clone().oneshot(request))
                .await
                .map_err(|_| format!("{path} blocked beyond five seconds"))?
                .map_err(|e| e.to_string())?;
        let status = response.status();
        let cookie = response
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.split(';').next().unwrap().to_owned());
        let bytes = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .map_err(|e| e.to_string())?;
        Ok((
            status,
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?,
            cookie,
        ))
    }

    pub(super) async fn pair(&self, label: &str) -> Check<Browser> {
        let (status, value, cookie) = self
            .request(
                "/api/v1/pair",
                None,
                json!({"code": "123456", "displayName": label}),
            )
            .await?;
        require(status == StatusCode::OK, "pairing failed")?;
        Ok(Browser {
            cookie: cookie.ok_or("pairing cookie absent")?,
            csrf: value["csrfToken"].as_str().ok_or("CSRF absent")?.into(),
        })
    }

    pub(super) async fn lease(&self, browser: &Browser, takeover: bool) -> Check<Value> {
        let (status, value, _) = self
            .request(
                &format!("/api/v1/projects/{}/lease", self.project_id),
                Some(browser),
                json!({"takeover": takeover}),
            )
            .await?;
        require(
            status == StatusCode::OK,
            "lease acquisition/renewal blocked or denied",
        )?;
        Ok(value)
    }

    pub(super) async fn rpc(
        &self,
        browser: &Browser,
        request: &RpcEnvelope,
    ) -> Check<(StatusCode, RpcResponse)> {
        let (status, value, _) = self
            .request(
                "/api/v1/rpc",
                Some(browser),
                serde_json::to_value(request).map_err(|e| e.to_string())?,
            )
            .await?;
        Ok((status, decode(value)?))
    }

    pub(super) async fn success(&self, browser: &Browser, request: &RpcEnvelope) -> Check<Value> {
        let (status, response) = self.rpc(browser, request).await?;
        require(
            status == StatusCode::OK && response.ok,
            &format!("{} failed: {response:?}", request.operation),
        )?;
        response.result.ok_or_else(|| "RPC result absent".into())
    }

    pub(super) async fn attempt(
        &self,
        browser: &Browser,
        job: &str,
        poll: u32,
    ) -> Check<MediaRenderAttempt> {
        decode(
            self.success(
                browser,
                &self.envelope(
                    &format!("attempt-{job}-{poll}"),
                    "load_render_attempt_in_split_project_folder",
                    None,
                    None,
                    json!({"jobId": job, "attemptId": format!("render-attempt/{job}")}),
                ),
            )
            .await?,
        )
    }

    pub(super) async fn terminal(&self, browser: &Browser, job: &str) -> Check<MediaRenderAttempt> {
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut poll = 100;
        loop {
            let result = self.attempt(browser, job, poll).await?;
            if result != MediaRenderAttempt::Pending {
                return Ok(result);
            }
            require(
                Instant::now() < deadline,
                "real worker did not reach a terminal attempt",
            )?;
            tokio::time::sleep(Duration::from_millis(500)).await;
            poll += 1;
        }
    }
    // Failure-only cleanup does not establish HTTP success. A timed-out spawn_blocking
    // admission can finish after its response future was dropped, so let it persist its
    // attempt after releasing our lock, then wait for its actual worker to settle.
    pub(super) async fn settle_after_failure(&self, job: &str) {
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(120) {
            match read_media_render_attempt(&self.path, job, &format!("render-attempt/{job}")) {
                Ok(MediaRenderAttempt::Pending) => {}
                Ok(_) => return,
                Err(_) if started.elapsed() < Duration::from_secs(5) => {}
                Err(_) => return,
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
