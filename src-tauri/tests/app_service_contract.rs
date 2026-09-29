use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::json;
use video_creater_lib::app_service::context::{ClientKind, RequestContext};
use video_creater_lib::app_service::error::{ServiceError, ServiceErrorCode};
use video_creater_lib::app_service::events::{EventSink, FakeEventSink, ServiceEvent};
use video_creater_lib::app_service::operation::AuthorizationScope;
use video_creater_lib::app_service::{ServiceDependencies, VideoCreaterService};

#[test]
fn request_context_redacts_lease_and_has_no_credentials() {
    let context = RequestContext::new(
        ClientKind::Browser,
        "request-7",
        Some("project-2".into()),
        Some(19),
        BTreeSet::from([
            AuthorizationScope::ProjectRead,
            AuthorizationScope::ProjectWrite,
        ]),
        Some("lease-secret-value".into()),
    )
    .expect("valid request context");

    let debug = format!("{context:?}");
    assert!(!debug.contains("lease-secret-value"));
    assert!(!debug.to_ascii_lowercase().contains("password"));
    assert!(!debug.to_ascii_lowercase().contains("credential"));
    assert!(context.allows(AuthorizationScope::ProjectRead));
    assert!(!context.allows(AuthorizationScope::HostAdmin));
}

#[test]
fn stable_errors_do_not_serialize_internal_details_or_absolute_paths() {
    let variants = [
        ServiceError::invalid_input("bad input at /home/user/project.json"),
        ServiceError::unavailable("rendering"),
        ServiceError::unauthorized(),
        ServiceError::forbidden(),
        ServiceError::revision_conflict(7, 8),
        ServiceError::lease_conflict(),
        ServiceError::not_found("project"),
        ServiceError::busy("render queue"),
        ServiceError::cancelled(),
        ServiceError::internal("database failed at /srv/private/project.json"),
    ];
    let expected_codes = [
        ServiceErrorCode::InvalidInput,
        ServiceErrorCode::UnavailableCapability,
        ServiceErrorCode::Unauthorized,
        ServiceErrorCode::Forbidden,
        ServiceErrorCode::RevisionConflict,
        ServiceErrorCode::LeaseConflict,
        ServiceErrorCode::NotFound,
        ServiceErrorCode::Busy,
        ServiceErrorCode::Cancelled,
        ServiceErrorCode::Internal,
    ];

    for (error, expected_code) in variants.into_iter().zip(expected_codes) {
        assert_eq!(error.code(), expected_code);
        let serialized = serde_json::to_string(&error).expect("serializable service error");
        assert!(!serialized.contains("/home/"), "{serialized}");
        assert!(!serialized.contains("/srv/"), "{serialized}");
        assert!(!serialized.contains("database failed"), "{serialized}");
        assert!(!serialized.contains("source"), "{serialized}");
    }
}

#[test]
fn fake_event_sink_assigns_deterministic_order() {
    let sink = FakeEventSink::default();
    let first = sink
        .publish(ServiceEvent::new(
            "job.progress",
            Some("project-2"),
            json!({"progress": 0.5}),
        ))
        .expect("first event");
    let second = sink
        .publish(ServiceEvent::new(
            "job.completed",
            Some("project-2"),
            json!({"ok": true}),
        ))
        .expect("second event");

    assert_eq!(first.sequence, 1);
    assert_eq!(second.sequence, 2);
    assert_eq!(sink.recorded(), vec![first, second]);
}

#[test]
fn service_owns_explicit_dependencies_and_an_event_sink() {
    let events: Arc<dyn EventSink> = Arc::new(FakeEventSink::default());
    let service = VideoCreaterService::new(ServiceDependencies::unconfigured(), events);

    assert_eq!(
        service.dependencies().project_store.name(),
        "unconfigured-project-store"
    );
    assert_eq!(
        service.dependencies().jobs.name(),
        "unconfigured-job-coordinator"
    );
    assert_eq!(
        service.dependencies().credentials.name(),
        "unconfigured-credential-store"
    );
    assert_eq!(
        service.dependencies().agents.name(),
        "unconfigured-agent-coordinator"
    );
    assert_eq!(
        service.dependencies().renders.name(),
        "unconfigured-render-coordinator"
    );
}
