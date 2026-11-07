use application::{IdentityRepository, Secrets};
use infrastructure::{Database, auth::SystemSecrets};
use uuid::Uuid;

#[tokio::test]
async fn confirmation_replays_share_a_mailbox_budget() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::migrate(&pool).await.unwrap();
    let db = Database::new(pool).unwrap();
    let link = SystemSecrets.token();
    db.issue_login(&format!("{}@example.test", Uuid::now_v7()), &link)
        .await
        .unwrap();
    db.consume_login(&link, &SystemSecrets.token())
        .await
        .unwrap();
    for _ in 0..4 {
        assert!(matches!(
            db.consume_login(&link, &SystemSecrets.token()).await,
            Err(domain::Error::Unauthenticated)
        ));
    }
    assert!(matches!(
        db.consume_login(&link, &SystemSecrets.token()).await,
        Err(domain::Error::RateLimited)
    ));
}

#[tokio::test]
async fn logout_waiting_on_rotation_retires_replacement() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::migrate(&pool).await.unwrap();
    let db = Database::new(pool.clone()).unwrap();
    let login = SystemSecrets.token();
    let old = SystemSecrets.token();
    let new = SystemSecrets.token();
    db.issue_login(&format!("{}@example.test", Uuid::now_v7()), &login)
        .await
        .unwrap();
    let actor = db.consume_login(&login, &old).await.unwrap();
    let mut rotation = pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *rotation)
        .await
        .unwrap();
    sqlx::query("UPDATE sessions SET token_hash=$1 WHERE id=$2")
        .bind(infrastructure::auth::token_hash(&new))
        .bind(actor.session_id.into_uuid())
        .execute(&mut *rotation)
        .await
        .unwrap();
    sqlx::query("INSERT INTO session_revocation_handles(token_hash,session_id) VALUES($1,$2)")
        .bind(infrastructure::auth::token_hash(&old))
        .bind(actor.session_id.into_uuid())
        .execute(&mut *rotation)
        .await
        .unwrap();
    let logout_db = db.clone();
    let replay = old.clone();
    let logout = tokio::spawn(async move { logout_db.revoke(&replay).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
            )
            .bind(pid)
            .fetch_one(&pool)
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    rotation.commit().await.unwrap();
    logout.await.unwrap().unwrap();
    assert!(
        db.authenticate(&new).await.is_err(),
        "replacement survived logout"
    );
    db.revoke(&old).await.unwrap();
}

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
    sqlx::query(
        "UPDATE sessions SET last_seen_at=clock_timestamp()-interval '31 minutes' WHERE id=$1",
    )
    .bind(actor.session_id.into_uuid())
    .execute(&db.pool)
    .await
    .unwrap();
    assert!(db.list_sessions(actor).await.unwrap().is_empty());
}
