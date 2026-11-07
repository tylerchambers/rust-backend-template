use application::{Authentication, LoginMailer};
use async_trait::async_trait;
use domain::Error;
use infrastructure::{Database, auth::SystemSecrets};
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
    infrastructure::migrate(&pool).await.unwrap();
    let db = Arc::new(Database::new(pool.clone()).unwrap());
    let mail = Arc::new(Capture::default());
    let auth = Authentication::new(db, mail.clone(), Arc::new(SystemSecrets));
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
    auth.request(&email).await.unwrap();
    let next = mail.0.lock().unwrap()[1].clone();
    assert_eq!(auth.confirm(&next).await.unwrap().user_id, session.user_id);
    auth.logout(&session.token).await.unwrap();
    assert!(matches!(
        auth.authenticate(&session.token).await,
        Err(Error::Unauthenticated)
    ));
    sqlx::query("UPDATE users SET disabled=true WHERE id=$1")
        .bind(session.user_id)
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
