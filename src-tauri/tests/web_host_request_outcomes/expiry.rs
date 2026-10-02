use super::*;

async fn expire_acknowledged_receipt() -> (tempfile::TempDir, Router, (String, String), String) {
    let root = tempdir().unwrap();
    let app = host(root.path(), 100);
    let credentials = pair(&app, "owner@example.test").await;
    let id = "browser-v2-100-expiry-receipt";
    assert!(
        rpc(&app, "owner@example.test", &credentials, creation(id))
            .await
            .ok
    );
    let ack = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/v1/request-outcomes/{id}/ack"),
            "owner@example.test",
            Some(&credentials.0),
            Some(&credentials.1),
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(ack.status(), StatusCode::OK);
    drop(app);
    let restarted = host(root.path(), 100 + 30 * 24 * 60 * 60 + 1);
    let replacement = pair(&restarted, "owner@example.test").await;
    (root, restarted, replacement, id.into())
}

#[tokio::test]
async fn pruned_expired_outcome_get_attests_a_replay_fence_after_restart() {
    let (_root, app, credentials, id) = expire_acknowledged_receipt().await;
    let response = app
        .oneshot(request(
            "GET",
            &format!("/api/v1/request-outcomes/{id}"),
            "owner@example.test",
            Some(&credentials.0),
            None,
            Value::Null,
        ))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::GONE,
        "a pruned expired identity needs an explicit reconciliation fence"
    );
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(
        body,
        json!({"error":"outcome_expired", "requestId":id, "replayFenced":true})
    );
}

#[tokio::test]
async fn pruned_expired_outcome_ack_confirms_the_fence_after_restart() {
    let (_root, app, credentials, id) = expire_acknowledged_receipt().await;
    let response = app
        .oneshot(request(
            "POST",
            &format!("/api/v1/request-outcomes/{id}/ack"),
            "owner@example.test",
            Some(&credentials.0),
            Some(&credentials.1),
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "reconciled expiry fence needs an explicit authenticated ACK"
    );
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(body, json!({"acknowledged":true}));
}

#[tokio::test]
async fn expiry_attestation_and_ack_preserve_authentication_and_foreign_retained_boundaries() {
    use video_creater_lib::web_host::request_outcome::RequestOutcomeStore;
    let root = tempdir().unwrap();
    let foreign = "browser-v2-100-foreign-retained";
    let store =
        RequestOutcomeStore::open(&root.path().join("sessions.request-outcomes"), 100).unwrap();
    let body = creation(foreign);
    store
        .begin(
            "other@example.test",
            &serde_json::from_value(body.clone()).unwrap(),
            &serde_json::to_vec(&body).unwrap(),
            100,
        )
        .unwrap();
    drop(store);
    let app = host(root.path(), 100 + 30 * 24 * 60 * 60 + 1);
    let owner = pair(&app, "owner@example.test").await;
    let absent = "browser-v2-100-never-accepted";
    let unauthorized = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/v1/request-outcomes/{absent}"),
            "owner@example.test",
            None,
            None,
            Value::Null,
        ))
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let invalid_csrf = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/v1/request-outcomes/{absent}/ack"),
            "owner@example.test",
            Some(&owner.0),
            Some("wrong-csrf"),
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(invalid_csrf.status(), StatusCode::FORBIDDEN);
    for id in [
        foreign,
        "missing-legacy",
        "browser-v2-bad-missing",
        "browser-v2-100-",
        "browser-v2-9999999999-future",
    ] {
        let get = app
            .clone()
            .oneshot(request(
                "GET",
                &format!("/api/v1/request-outcomes/{id}"),
                "owner@example.test",
                Some(&owner.0),
                None,
                Value::Null,
            ))
            .await
            .unwrap();
        assert_eq!(get.status(), StatusCode::NOT_FOUND, "must not attest {id}");
        let ack = app
            .clone()
            .oneshot(request(
                "POST",
                &format!("/api/v1/request-outcomes/{id}/ack"),
                "owner@example.test",
                Some(&owner.0),
                Some(&owner.1),
                json!({}),
            ))
            .await
            .unwrap();
        assert_eq!(
            ack.status(),
            StatusCode::NOT_FOUND,
            "must not acknowledge missing {id}"
        );
    }
    let attestation = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/v1/request-outcomes/{absent}"),
            "owner@example.test",
            Some(&owner.0),
            None,
            Value::Null,
        ))
        .await
        .unwrap();
    assert_eq!(attestation.status(), StatusCode::GONE);
    let ack = app
        .oneshot(request(
            "POST",
            &format!("/api/v1/request-outcomes/{absent}/ack"),
            "owner@example.test",
            Some(&owner.0),
            Some(&owner.1),
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(ack.status(), StatusCode::OK);
}
