#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::Authentication;
use application::authentication::LoginMailer;
use async_trait::async_trait;
use domain::Error;
use infrastructure::postgres::Database;
use infrastructure::system::SystemSecrets;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
#[derive(Default)]
struct Capture(Mutex<Vec<String>>);
#[async_trait]
impl LoginMailer for Capture {
    async fn send_login(&self, _email: &str, token: &str) -> Result<(), Error> {
        self.0.lock().unwrap().push(token.into());
        Ok(())
    }
}
#[tokio::test]
async fn magic_tokens_are_single_use_identity_is_unique_and_logout_revokes() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
    let db = Arc::new(Database::new(pool.clone()).unwrap());
    let mail = Arc::new(Capture::default());
    let auth = Authentication::new(db.clone(), mail.clone(), Arc::new(SystemSecrets));
    let memberships = application::invitations::Invitations::new(
        db,
        Arc::new(infrastructure::system::SystemIds),
        Arc::new(infrastructure::system::SystemClock),
    );
    let actor = application::Actor {
        user_id: Uuid::now_v7().into(),
        session_id: Uuid::now_v7().into(),
        correlation_id: Uuid::now_v7(),
    };
    for invalid in ["@", "a@@b", "a@", "@example.test", "a..b@example.test"] {
        let before: (i64, Option<i32>) = sqlx::query_as("SELECT (SELECT count(*) FROM magic_links WHERE email=$1), (SELECT attempts FROM auth_limits WHERE key_hash=$2)")
            .bind(invalid).bind(infrastructure::postgres::token_hash(invalid)).fetch_one(&pool).await.unwrap();
        assert_eq!(auth.request(invalid).await, Err(Error::Invalid));
        let after: (i64, Option<i32>) = sqlx::query_as("SELECT (SELECT count(*) FROM magic_links WHERE email=$1), (SELECT attempts FROM auth_limits WHERE key_hash=$2)")
            .bind(invalid).bind(infrastructure::postgres::token_hash(invalid)).fetch_one(&pool).await.unwrap();
        assert_eq!(
            before, after,
            "invalid mailbox mutated credentials or budget"
        );
        assert_eq!(
            memberships
                .invite(
                    &actor,
                    Uuid::now_v7().into(),
                    invalid,
                    domain::Role::Member,
                    "invalid"
                )
                .await,
            Err(Error::Invalid)
        );
    }
    assert!(mail.0.lock().unwrap().is_empty());
    let email = format!("{}@example.test", Uuid::now_v7());
    auth.request(&email).await.unwrap();
    let token = mail.0.lock().unwrap()[0].clone();
    let (a, b) = tokio::join!(auth.confirm(&token), auth.confirm(&token));
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let session = a.or(b).unwrap();
    assert_eq!(
        auth.authenticate(&session.token).await.unwrap().user_id,
        session.user_id
    );
    auth.request(&format!(" {} ", email.to_ascii_uppercase()))
        .await
        .unwrap();
    let next = mail.0.lock().unwrap()[1].clone();
    assert_eq!(auth.confirm(&next).await.unwrap().user_id, session.user_id);
    auth.logout(&session.token).await.unwrap();
    assert!(matches!(
        auth.authenticate(&session.token).await,
        Err(Error::Unauthenticated)
    ));
    sqlx::query("UPDATE users SET disabled=true WHERE id=$1")
        .bind(session.user_id.into_uuid())
        .execute(&pool)
        .await
        .unwrap();
    auth.request(&email).await.unwrap();
    let blocked = mail.0.lock().unwrap()[2].clone();
    assert!(matches!(
        auth.confirm(&blocked).await,
        Err(Error::Unauthenticated)
    ));
    assert!(auth.confirm("invalid").await.is_err());
}
