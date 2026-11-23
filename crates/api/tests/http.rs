#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::Secrets;
use application::{
    Authentication, Organizations, Projects,
    authentication::{IdentityRepository, LoginMailer},
    reauthentication::{ProofPurpose, ReauthenticationMailer},
};
use async_trait::async_trait;
use axum::{body::Body, http::Request};
use domain::{Error, OrganizationId};
use infrastructure::postgres::Database;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
struct Mail;
#[async_trait]
impl LoginMailer for Mail {
    async fn send_login(&self, _: &str, _: &str) -> Result<(), Error> {
        Ok(())
    }
}

#[derive(Default)]
struct ProofMailbox(Mutex<Vec<(ProofPurpose, OrganizationId)>>);

#[async_trait]
impl ReauthenticationMailer for ProofMailbox {
    async fn send_proof(
        &self,
        _: &str,
        purpose: ProofPurpose,
        organization: OrganizationId,
        _: &str,
    ) -> Result<(), Error> {
        self.0.lock().unwrap().push((purpose, organization));
        Ok(())
    }
}

fn test_app(db: &Arc<Database>, proof_mail: Arc<ProofMailbox>) -> axum::Router {
    let secrets = Arc::new(infrastructure::system::SystemSecrets);
    let state = api::State::new(
        Arc::new(Authentication::new(
            db.clone(),
            Arc::new(Mail),
            secrets.clone(),
        )),
        Arc::new(Organizations::new(
            db.clone(),
            std::sync::Arc::new(infrastructure::system::SystemIds),
        )),
        Arc::new(Projects::new(
            db.clone(),
            std::sync::Arc::new(infrastructure::system::SystemIds),
        )),
        secrets.clone(),
        "http://localhost:58039".into(),
        Arc::new(application::memberships::Memberships::new(db.clone())),
        Arc::new(application::invitations::Invitations::new(
            db.clone(),
            std::sync::Arc::new(infrastructure::system::SystemIds),
            std::sync::Arc::new(infrastructure::system::SystemClock),
        )),
        Arc::new(application::reauthentication::Reauthentication::new(
            db.clone(),
            proof_mail,
            secrets,
        )),
        Arc::new(application::administration::Administration::new(db.clone())),
    );
    api::router(state).layer(axum::Extension(axum::extract::ConnectInfo(
        std::net::SocketAddr::new(
            std::net::IpAddr::V6(std::net::Ipv6Addr::from(uuid::Uuid::now_v7().as_u128())),
            1234,
        ),
    )))
}

#[tokio::test]
async fn proof_requests_preserve_bindings_and_reject_a_mismatched_invite_before_issuance() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
    let db = Arc::new(Database::new(pool.clone()).unwrap());
    let mailbox = Arc::new(ProofMailbox::default());
    let app = test_app(&db, mailbox.clone());
    let login = infrastructure::system::SystemSecrets.token();
    let session = infrastructure::system::SystemSecrets.token();
    db.issue_login(&format!("{}@example.test", uuid::Uuid::now_v7()), &login)
        .await
        .unwrap();
    let actor = db.consume_login(&login, &session).await.unwrap();
    let org = Organizations::new(db, Arc::new(infrastructure::system::SystemIds))
        .create(&actor, "Proof requests".into())
        .await
        .unwrap();
    let other = uuid::Uuid::now_v7();
    let csrf = "a".repeat(64);
    for (action, target, expected) in [
        ("invite", other, None),
        ("unknown", other, None),
        ("invite", org.into_uuid(), Some(ProofPurpose::Invite)),
        (
            "change_role",
            other,
            Some(ProofPurpose::ChangeRole(other.into())),
        ),
        (
            "remove_member",
            other,
            Some(ProofPurpose::RemoveMember(other.into())),
        ),
        (
            "revoke_invitation",
            other,
            Some(ProofPurpose::RevokeInvitation(other.into())),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/auth/reauth/request")
                    .header("origin", "http://localhost:58039")
                    .header("cookie", format!("csrf={csrf}; __Host-session={session}"))
                    .header("x-csrf-token", &csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "action": action, "target": target, "organization_id": org,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        if let Some(purpose) = expected {
            assert_eq!(response.status(), 202);
            let mut delivered = mailbox.0.lock().unwrap();
            assert_eq!(*delivered, [(purpose, org)]);
            delivered.clear();
        } else {
            assert_eq!(response.status(), 400);
            assert!(mailbox.0.lock().unwrap().is_empty());
            let issued: i64 =
                sqlx::query_scalar("SELECT count(*) FROM reauthentication_proofs WHERE user_id=$1")
                    .bind(actor.user_id.into_uuid())
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(issued, 0);
        }
    }
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "One ordered integration journey retains shared state and security assertions"
)]
async fn scanner_get_does_not_authenticate_and_mutations_require_csrf() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
    let db = Arc::new(Database::new(pool.clone()).unwrap());
    let app = test_app(&db, Arc::new(ProofMailbox::default()));
    let contract: serde_json::Value = serde_json::from_str(&api::openapi()).unwrap();
    for (method, path, status, code) in [
        ("GET", "/v1/auth/sessions", 401, "unauthenticated"),
        ("POST", "/v1/organizations", 403, "forbidden"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(response.headers()["content-type"], "application/json");
        let bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let error: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error, serde_json::json!({"code": code}));
        assert_eq!(
            contract["paths"][path][method.to_lowercase()]["responses"]["default"]["content"]["application/json"]
                ["schema"]["$ref"],
            "#/components/schemas/ErrorBody"
        );
    }
    let login = infrastructure::system::SystemSecrets.token();
    let session = infrastructure::system::SystemSecrets.token();
    db.issue_login(&format!("{}@example.test", uuid::Uuid::now_v7()), &login)
        .await
        .unwrap();
    let owner = db.consume_login(&login, &session).await.unwrap();
    let org = Organizations::new(db.clone(), Arc::new(infrastructure::system::SystemIds))
        .create(&owner, "Discoverable".into())
        .await
        .unwrap();
    // All sessions share a timestamp: pagination must use unique IDs, not time.
    let mut expected = vec![owner.session_id.into_uuid()];
    for index in 0..105 {
        let id = uuid::Uuid::now_v7();
        sqlx::query(
            "INSERT INTO sessions(id,user_id,token_hash,created_at) VALUES($1,$2,$3,'2026-01-01')",
        )
        .bind(id)
        .bind(owner.user_id.into_uuid())
        .bind(infrastructure::postgres::token_hash(&format!(
            "{session}-{index}"
        )))
        .execute(&pool)
        .await
        .unwrap();
        expected.push(id);
    }
    let hidden = expected[1];
    sqlx::query("UPDATE sessions SET created_at='2025-01-01' WHERE id=$1")
        .bind(hidden)
        .execute(&pool)
        .await
        .unwrap();
    let foreign = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,email) VALUES($1,$2)")
        .bind(foreign)
        .bind(format!("{foreign}@example.test"))
        .execute(&pool)
        .await
        .unwrap();
    for (index, user, state) in [
        (0, foreign, "live"),
        (1, owner.user_id.into_uuid(), "expired"),
        (2, owner.user_id.into_uuid(), "revoked"),
        (3, owner.user_id.into_uuid(), "idle"),
    ] {
        sqlx::query("INSERT INTO sessions(id,user_id,token_hash,expires_at,revoked_at,last_seen_at) VALUES($1,$2,$3,CASE WHEN $4='expired' THEN clock_timestamp()-interval '1 day' ELSE clock_timestamp()+interval '1 day' END,CASE WHEN $4='revoked' THEN clock_timestamp() END,CASE WHEN $4='idle' THEN clock_timestamp()-interval '1 hour' ELSE clock_timestamp() END)")
            .bind(uuid::Uuid::now_v7()).bind(user)
            .bind(infrastructure::postgres::token_hash(&format!("{session}-excluded-{index}")))
            .bind(state).execute(&pool).await.unwrap();
    }
    let mut collected = Vec::new();
    let mut terminated = false;
    let mut path = "/v1/auth/sessions".to_owned();
    for _ in 0..5 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&path)
                    .header("cookie", format!("__Host-session={session}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let bytes = axum::body::to_bytes(response.into_body(), 16384)
            .await
            .unwrap();
        let rows: Vec<uuid::Uuid> = serde_json::from_slice(&bytes).unwrap();
        assert!(rows.len() <= 50, "session pages must remain bounded");
        if rows.is_empty() {
            terminated = true;
            break;
        }
        path = format!("/v1/auth/sessions?after={}", rows.last().unwrap());
        collected.extend(rows);
    }
    assert!(
        terminated,
        "session traversal must reach an empty terminal page"
    );
    expected.sort_unstable();
    assert_eq!(
        collected, expected,
        "all live sessions exactly once in UUID order"
    );
    let csrf = "a".repeat(64);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/auth/sessions/{hidden}/revoke"))
                .header("origin", "http://localhost:58039")
                .header("x-csrf-token", &csrf)
                .header("cookie", format!("csrf={csrf}; __Host-session={session}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    let revoked: bool =
        sqlx::query_scalar("SELECT revoked_at IS NOT NULL FROM sessions WHERE id=$1")
            .bind(hidden)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(revoked);
    for path in [
        "/v1/organizations".to_string(),
        format!("/v1/organizations/{org}/members"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&path)
                    .header("x-request-id", "untrusted-client-id")
                    .header("cookie", format!("__Host-session={session}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200, "discovery route {path}");
        let request_id = response.headers()["x-request-id"].to_str().unwrap();
        assert_ne!(request_id, "untrusted-client-id");
        assert_eq!(
            uuid::Uuid::parse_str(request_id).unwrap().get_version_num(),
            7
        );
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["referrer-policy"], "no-referrer");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        let body = axum::body::to_bytes(response.into_body(), 16384)
            .await
            .unwrap();
        let rows: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 1);
        assert_eq!(rows[0]["role"], "owner");
    }
    for cookies in [
        vec![format!(
            "__Host-session={session}; __Host-session={session}"
        )],
        vec![
            format!("__Host-session={session}"),
            format!("__Host-session={session}"),
        ],
    ] {
        let mut request = Request::builder().uri("/v1/auth/sessions");
        for value in cookies {
            request = request.header("cookie", value);
        }
        assert_eq!(
            app.clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            401,
            "ambiguous session cookies must not authenticate"
        );
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/auth/confirmation?token=scanner")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.starts_with("csrf="));
    assert!(!cookie.contains("session="));
    let csrf = cookie
        .split(';')
        .next()
        .unwrap()
        .strip_prefix("csrf=")
        .unwrap();
    for (origin, token) in [
        ("https://foreign.example", csrf),
        ("http://localhost:58039", "incorrect"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/confirm")
                    .header("origin", origin)
                    .header("x-csrf-token", token)
                    .header("cookie", format!("csrf={csrf}"))
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 403);
    }
    for ambiguous in ["origin", "x-csrf-token", "cookie"] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/v1/auth/logout")
            .header("origin", "http://localhost:58039")
            .header("x-csrf-token", csrf)
            .header("cookie", format!("csrf={csrf}; __Host-session={session}"));
        request = request.header(
            ambiguous,
            match ambiguous {
                "origin" => "http://localhost:58039".to_owned(),
                "x-csrf-token" => csrf.to_owned(),
                _ => format!("csrf={csrf}"),
            },
        );
        assert_eq!(
            app.clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            403
        );
    }
    let malformed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/confirm")
                .header("content-type", "application/json")
                .header("origin", "http://localhost:58039")
                .header("cookie", format!("csrf={csrf}"))
                .header("x-csrf-token", csrf)
                .body(Body::from("{"))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(malformed.into_body(), 4096)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["code"],
        "invalid_input"
    );
    // Distinct mailboxes still consume the same trusted socket budget.
    for _ in 0..60 {
        let response = app.clone().oneshot(Request::builder().method("POST")
            .uri("/v1/auth/request").header("content-type", "application/json")
            .header("origin", "http://localhost:58039").header("cookie", format!("csrf={csrf}"))
            .header("x-csrf-token", csrf).header("x-forwarded-for", uuid::Uuid::now_v7().to_string())
            .body(Body::from(serde_json::json!({"email":format!("{}@example.test",uuid::Uuid::now_v7())}).to_string())).unwrap()).await.unwrap();
        assert_eq!(response.status(), 202);
    }
    let mut limited = false;
    for _ in 0..125 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/confirm")
                    .header("content-type", "application/json")
                    .header("origin", "http://localhost:58039")
                    .header("cookie", format!("csrf={csrf}"))
                    .header("x-csrf-token", csrf)
                    .header("x-forwarded-for", uuid::Uuid::now_v7().to_string())
                    .body(Body::from(
                        serde_json::json!({"token":"f".repeat(64)}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        if response.status() == 429 {
            limited = true;
            break;
        }
        assert_eq!(response.status(), 401);
    }
    assert!(
        limited,
        "confirmation guessing must be limited by socket IP, not forwarded headers"
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/confirm")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"token":"invalid"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/organizations/00000000-0000-0000-0000-000000000001/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/metrics/http")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let metrics = std::str::from_utf8(&body).unwrap();
    assert!(metrics.contains("# TYPE http_request_duration_seconds histogram"));
    assert!(metrics.contains("http_request_duration_seconds_bucket{le=\"+Inf\"}"));
    assert!(metrics.ends_with("# EOF\n"));
    assert!(metrics.contains("http_request_duration_seconds_sum "));
    assert!(metrics.contains("authentication_rate_limited_total 1"));
    let post_request = |path: &str| {
        Request::builder()
            .method("POST")
            .uri(path)
            .header("origin", "http://localhost:58039")
            .header("cookie", format!("csrf={csrf}; __Host-session={session}"))
            .header("x-csrf-token", csrf)
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap()
    };
    let limited = app
        .clone()
        .oneshot(post_request("/v1/auth/request"))
        .await
        .unwrap();
    assert_eq!(
        limited.status(),
        429,
        "initiation must share socket admission even for new addresses"
    );
    // Hold the rotated response in the test instead of delivering its cookie to
    // the browser; logout still arrives with the original cookie.
    let rotated = app
        .clone()
        .oneshot(post_request("/v1/auth/sessions/rotate"))
        .await
        .unwrap();
    assert_eq!(rotated.status(), 204);
    let replacement = rotated
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let replay = || {
        Request::builder()
            .uri("/v1/auth/sessions")
            .header("cookie", replacement)
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        app.clone().oneshot(replay()).await.unwrap().status(),
        200,
        "live no-logout control"
    );
    assert_eq!(
        app.clone()
            .oneshot(post_request("/v1/auth/logout"))
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        app.oneshot(replay()).await.unwrap().status(),
        401,
        "delayed replacement cookie survived logout"
    );
}
