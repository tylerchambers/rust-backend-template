use application::{IdentityRepository, Secrets};
use infrastructure::{Database, auth::SystemSecrets};
use uuid::Uuid;
#[tokio::test]
async fn rotation_retires_old_token_and_account_revocation_retires_all() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::migrate(&pool).await.unwrap();
    let db = Database::new(pool).unwrap();
    let secrets = SystemSecrets;
    let login = secrets.token();
    let old = secrets.token();
    let new = secrets.token();
    db.issue_login(&format!("{}@example.test", Uuid::now_v7()), &login)
        .await
        .unwrap();
    let actor = db.consume_login(&login, &old).await.unwrap();
    assert_eq!(db.list_sessions(&actor).await.unwrap().len(), 1);
    db.rotate(&actor, &old, &new).await.unwrap();
    assert!(db.authenticate(&old).await.is_err());
    assert!(db.authenticate(&new).await.is_ok());
    db.revoke_all(&actor).await.unwrap();
    assert!(db.authenticate(&new).await.is_err());
}

#[tokio::test]
async fn targeted_revocation_is_owner_scoped_and_preserves_other_sessions() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::migrate(&pool).await.unwrap();
    let db = Database::new(pool).unwrap();
    let email = format!("{}@example.test", Uuid::now_v7());
    let mut identities = Vec::new();
    for address in [
        email.clone(),
        email,
        format!("{}@example.test", Uuid::now_v7()),
    ] {
        let link = SystemSecrets.token();
        let token = SystemSecrets.token();
        db.issue_login(&address, &link).await.unwrap();
        identities.push((db.consume_login(&link, &token).await.unwrap(), token));
    }
    let (actor, first) = &identities[0];
    let (second, second_token) = &identities[1];
    let (stranger, stranger_token) = &identities[2];
    assert!(db.revoke_session(actor, stranger.session_id).await.is_err());
    assert!(db.authenticate(stranger_token).await.is_ok());
    db.revoke_session(actor, second.session_id).await.unwrap();
    assert!(db.authenticate(second_token).await.is_err());
    assert!(db.authenticate(first).await.is_ok());
    assert_eq!(
        db.list_sessions(actor).await.unwrap(),
        vec![actor.session_id]
    );
}
