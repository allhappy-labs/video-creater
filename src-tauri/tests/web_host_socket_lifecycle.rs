use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::http::header;
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use video_creater_lib::project::{
    action::ProjectAction, model::VideoProject, split::save_split_project,
};
use video_creater_lib::web_host::{
    dispatcher::HostDispatcher,
    http::{host_router, HostHttpConfig},
    project_catalog::ProjectCatalog,
    session::SessionStore,
};

const ORIGIN: &str = "https://socket-lifecycle.tail.test";

struct Browser {
    cookie: String,
    csrf: String,
    session_id: String,
}

struct Host {
    _root: TempDir,
    server: tokio::task::JoinHandle<()>,
    address: SocketAddr,
    client: reqwest::Client,
    project_id: String,
    project: VideoProject,
}

impl Drop for Host {
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl Host {
    async fn start(short_session: bool) -> (Self, Option<Browser>) {
        let root = tempfile::tempdir().unwrap();
        let session_file = root.path().join("sessions.json");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let expiring = short_session.then(|| {
            let store = SessionStore::load_at(session_file.clone(), ORIGIN, 2, now).unwrap();
            let issued = store
                .issue("Expiring", Some("remote-e2e@localhost"), now)
                .unwrap();
            Browser {
                cookie: issued.cookie_header.split(';').next().unwrap().to_owned(),
                csrf: issued.csrf_token,
                session_id: issued.session_id,
            }
        });
        let project_root = root.path().join("projects");
        std::fs::create_dir_all(&project_root).unwrap();
        let project_dir = project_root.join("project.palmier");
        let project = save_split_project(
            &project_dir,
            &VideoProject::new_empty(
                "socket-project".into(),
                "Before".into(),
                "2026-10-03T00:00:00Z".into(),
            ),
        )
        .unwrap()
        .project;
        let catalog = ProjectCatalog::new(vec![project_root]).unwrap();
        let project_id = catalog.id_for_path(&project_dir).unwrap();
        let app = host_router(
            HostHttpConfig {
                assets_dir: root.path().to_owned(),
                session_file,
                public_origin: ORIGIN.into(),
                host_label: "Lifecycle test".into(),
                pairing_code: "123456".into(),
                reusable_pairing_code: true,
                issued_at: now,
                now_override: None,
                uploads_dir: root.path().join("uploads"),
                upload_max_bytes: 1024,
                upload_reserve_bytes: 0,
                project_catalog: catalog.clone(),
                allow_direct_test_identity: true,
            },
            Arc::new(
                HostDispatcher::with_project_catalog(catalog)
                    .with_settings_root(root.path().join("settings-state")),
            ),
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
        (
            Self {
                _root: root,
                server,
                address,
                client: reqwest::Client::new(),
                project_id,
                project,
            },
            expiring,
        )
    }

    async fn post(&self, browser: Option<&Browser>, path: &str, body: Value) -> reqwest::Response {
        let mut request = self
            .client
            .post(format!("http://{}{}", self.address, path))
            .header(header::ORIGIN, ORIGIN)
            .json(&body);
        if let Some(browser) = browser {
            request = request
                .header(header::COOKIE, &browser.cookie)
                .header("x-csrf-token", &browser.csrf);
        }
        request.send().await.unwrap()
    }

    async fn pair(&self) -> Browser {
        let response = self
            .post(
                None,
                "/api/v1/pair",
                json!({"code":"123456","displayName":"Test"}),
            )
            .await;
        assert!(response.status().is_success());
        let cookie = response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        let value: Value = response.json().await.unwrap();
        Browser {
            cookie,
            csrf: value["csrfToken"].as_str().unwrap().into(),
            session_id: value["sessionId"].as_str().unwrap().into(),
        }
    }

    async fn socket(&self, browser: &Browser) -> TcpStream {
        let mut socket = TcpStream::connect(self.address).await.unwrap();
        socket.write_all(format!(
            "GET /api/v1/events HTTP/1.1\r\nHost: {}\r\nOrigin: {}\r\nCookie: {}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Protocol: vc-events, vc-csrf.{}, vc-resume.0\r\n\r\n",
            self.address, ORIGIN, browser.cookie, browser.csrf,
        ).as_bytes()).await.unwrap();
        let mut response = Vec::new();
        while !response.ends_with(b"\r\n\r\n") {
            response.push(socket.read_u8().await.unwrap());
            assert!(response.len() < 8192);
        }
        assert!(response.starts_with(b"HTTP/1.1 101 "));
        socket
    }

    async fn lease(&self, browser: &Browser) -> String {
        let response = self
            .post(
                Some(browser),
                &format!("/api/v1/projects/{}/lease", self.project_id),
                json!({}),
            )
            .await;
        assert!(response.status().is_success());
        let lease: Value = response.json().await.unwrap();
        lease["editorLeaseToken"].as_str().unwrap().into()
    }

    async fn mutate(&self, browser: &Browser) {
        let token = self.lease(browser).await;
        self.mutate_with_token(browser, &token).await;
    }

    async fn mutate_with_token(&self, browser: &Browser, token: &str) {
        let response = self
            .post(
                Some(browser),
                "/api/v1/rpc",
                json!({
                    "requestId":uuid::Uuid::new_v4().to_string(),
                    "operation":"apply_project_action_to_split_project_folder",
                    "projectId":self.project_id, "expectedRevision":self.project.content_revision,
                    "editorLeaseToken":token,
                    "payload":{"action":ProjectAction::UpdateProjectSettings {
                        name:"After".into(), render_settings:self.project.render_settings.clone(),
                    }},
                }),
            )
            .await;
        assert!(response.status().is_success());
        let result: Value = response.json().await.unwrap();
        assert_eq!(result["ok"], true);
    }
}

async fn frame(socket: &mut TcpStream, deadline: Duration) -> Option<(u8, Vec<u8>)> {
    tokio::time::timeout(deadline, async {
        let first = socket.read_u8().await.ok()?;
        let second = socket.read_u8().await.ok()?;
        let length = match second & 127 {
            126 => socket.read_u16().await.ok()? as u64,
            127 => socket.read_u64().await.ok()?,
            length => length as u64,
        };
        assert_eq!(second & 128, 0);
        assert!(length < 1024 * 1024);
        let mut payload = vec![0; length as usize];
        socket.read_exact(&mut payload).await.ok()?;
        Some((first & 15, payload))
    })
    .await
    .expect("socket did not terminate within the deadline")
}

async fn frame_opcode(socket: &mut TcpStream, deadline: Duration) -> Option<u8> {
    frame(socket, deadline).await.map(|(opcode, _)| opcode)
}

async fn assert_closed(socket: &mut TcpStream, deadline: Duration) {
    assert!(matches!(
        frame_opcode(socket, deadline).await,
        None | Some(8)
    ));
}

#[tokio::test]
async fn revoked_socket_cannot_receive_later_project_results() {
    let (host, _) = Host::start(false).await;
    let first = host.pair().await;
    let second = host.pair().await;
    let mut old_socket = host.socket(&first).await;
    let mut valid_socket = host.socket(&second).await;
    assert!(host
        .post(
            Some(&second),
            &format!("/api/v1/admin/sessions/{}/revoke", first.session_id),
            json!({})
        )
        .await
        .status()
        .is_success());
    host.mutate(&second).await;
    assert_eq!(
        frame_opcode(&mut valid_socket, Duration::from_secs(2)).await,
        Some(1)
    );
    assert_closed(&mut old_socket, Duration::from_secs(2)).await;
}

#[tokio::test]
async fn logout_rotation_and_revoke_all_close_idle_existing_sockets() {
    for endpoint in [
        "/api/v1/session/logout",
        "/api/v1/session/rotate",
        "/api/v1/admin/sessions/revoke-all",
    ] {
        let (host, _) = Host::start(false).await;
        let browser = host.pair().await;
        let mut socket = host.socket(&browser).await;
        assert!(host
            .post(Some(&browser), endpoint, json!({}))
            .await
            .status()
            .is_success());
        assert_closed(&mut socket, Duration::from_secs(2)).await;
    }
}

#[tokio::test]
async fn expired_session_closes_an_idle_existing_socket() {
    let (host, browser) = Host::start(true).await;
    let mut socket = host.socket(&browser.unwrap()).await;
    assert_closed(&mut socket, Duration::from_secs(5)).await;
}

#[tokio::test]
async fn closing_one_of_two_sockets_does_not_expire_the_remaining_editor() {
    let (host, _) = Host::start(false).await;
    let browser = host.pair().await;
    let first = host.socket(&browser).await;
    let mut second = host.socket(&browser).await;
    let token = host.lease(&browser).await;
    drop(first);
    tokio::time::sleep(Duration::from_secs(11)).await;
    host.mutate_with_token(&browser, &token).await;
    assert_eq!(
        frame_opcode(&mut second, Duration::from_secs(2)).await,
        Some(1)
    );
}

#[tokio::test]
async fn settings_operations_publish_terminal_events_to_authenticated_websockets() {
    let (host, _) = Host::start(false).await;
    let browser = host.pair().await;
    let mut socket = host.socket(&browser).await;
    let response = host.post(Some(&browser), "/api/v1/rpc", json!({
        "requestId":"settings-events", "operation":"run_agent_component_self_test", "payload":{"componentId":"agent.proposalValidator"}
    })).await;
    assert!(response.status().is_success());
    let mut states = Vec::new();
    for _ in 0..8 {
        let (opcode, payload) = frame(&mut socket, Duration::from_secs(5)).await.unwrap();
        assert_eq!(opcode, 1);
        let event: Value = serde_json::from_slice(&payload).unwrap();
        if event["topic"] != "settings-operation" {
            continue;
        }
        assert_eq!(event["payload"]["targetId"], "agent.proposalValidator");
        states.push(event["payload"]["state"].as_str().unwrap().to_owned());
        if states.last().is_some_and(|state| state == "succeeded") {
            break;
        }
    }
    assert_eq!(states, ["queued", "running", "succeeded"]);
}
