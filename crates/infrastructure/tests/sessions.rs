#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::Secrets;
use application::authentication::IdentityRepository;
use infrastructure::postgres::Database;
use infrastructure::system::SystemSecrets;
use uuid::Uuid;

async fn session(db: &Database, email: &str) -> (application::Actor, String) {
    let link = SystemSecrets.token();
    let token = SystemSecrets.token();
    db.issue_login(email, &link).await.unwrap();
    (db.consume_login(&link, &token).await.unwrap(), token)
}

async fn revocation_record(
    db: &Database,
    id: domain::SessionId,
) -> (chrono::DateTime<chrono::Utc>, i64) {
    sqlx::query_as(
        "SELECT revoked_at, (SELECT count(*) FROM identity_audit
         WHERE session_id=$1 AND action='session.revoked') FROM sessions WHERE id=$1",
    )
    .bind(id.into_uuid())
    .fetch_one(&db.pool)
    .await
    .unwrap()
}

async fn waiting_session(pool: &sqlx::PgPool, blocker: i32, exclude: i32) -> i32 {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let waiting = sqlx::query_scalar(
                "SELECT pid FROM pg_stat_activity WHERE pid<>$2
                 AND ($1=ANY(pg_blocking_pids(pid)) OR $2=ANY(pg_blocking_pids(pid))) LIMIT 1",
            )
            .bind(blocker)
            .bind(exclude)
            .fetch_optional(pool)
            .await
            .unwrap();
            if let Some(pid) = waiting {
                return pid;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("operation waited for the database lock")
}

#[tokio::test]
async fn confirmation_replays_share_a_mailbox_budget() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
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
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
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
        .bind(infrastructure::postgres::token_hash(&new))
        .bind(actor.session_id.into_uuid())
        .execute(&mut *rotation)
        .await
        .unwrap();
    sqlx::query("INSERT INTO session_revocation_handles(token_hash,session_id) VALUES($1,$2)")
        .bind(infrastructure::postgres::token_hash(&old))
        .bind(actor.session_id.into_uuid())
        .execute(&mut *rotation)
        .await
        .unwrap();
    let logout_db = db.clone();
    let replay = old.clone();
    let logout = tokio::spawn(async move { logout_db.revoke(&replay).await });
    waiting_session(&pool, pid, 0).await;
    rotation.commit().await.unwrap();
    logout.await.unwrap().unwrap();
    assert!(
        db.authenticate(&new).await.is_err(),
        "replacement survived logout"
    );
    let record = revocation_record(&db, actor.session_id).await;
    assert_eq!(record.1, 1);
    db.revoke(&old).await.unwrap();
    assert_eq!(revocation_record(&db, actor.session_id).await, record);
}

#[tokio::test]
async fn rotation_retires_old_token_and_account_revocation_retires_all() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
    let db = Database::new(pool).unwrap();
    let secrets = SystemSecrets;
    let login = secrets.token();
    let old = secrets.token();
    let new = secrets.token();
    db.issue_login(&format!("{}@example.test", Uuid::now_v7()), &login)
        .await
        .unwrap();
    let actor = db.consume_login(&login, &old).await.unwrap();
    assert_eq!(db.list_sessions(&actor, None).await.unwrap().len(), 1);
    db.rotate(&actor, &old, &new).await.unwrap();
    assert!(db.authenticate(&old).await.is_err());
    assert_eq!(
        db.revoke_all(&old).await,
        Err(domain::Error::Unauthenticated)
    );
    assert!(db.authenticate(&new).await.is_ok());
    db.revoke_all(&new).await.unwrap();
    assert!(db.authenticate(&new).await.is_err());
    assert_eq!(
        db.revoke_all(&new).await,
        Err(domain::Error::Unauthenticated)
    );
}

#[tokio::test]
async fn targeted_revocation_is_owner_scoped_and_preserves_other_sessions() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
    let db = Database::new(pool).unwrap();
    let email = format!("{}@example.test", Uuid::now_v7());
    let mut identities = Vec::new();
    for address in [
        email.clone(),
        email,
        format!("{}@example.test", Uuid::now_v7()),
    ] {
        identities.push(session(&db, &address).await);
    }
    let (actor, first) = &identities[0];
    let (second, second_token) = &identities[1];
    let (stranger, stranger_token) = &identities[2];
    assert_eq!(
        db.revoke_session(actor, stranger.session_id).await,
        Err(domain::Error::NotFound)
    );
    assert!(db.authenticate(stranger_token).await.is_ok());
    db.revoke_session(actor, second.session_id).await.unwrap();
    assert!(db.authenticate(second_token).await.is_err());
    let record = revocation_record(&db, second.session_id).await;
    assert_eq!(record.1, 1);
    db.revoke_session(actor, second.session_id).await.unwrap();
    assert_eq!(revocation_record(&db, second.session_id).await, record);
    assert!(db.authenticate(first).await.is_ok());
    assert_eq!(
        db.list_sessions(actor, None).await.unwrap(),
        vec![actor.session_id]
    );
    sqlx::query(
        "UPDATE sessions SET last_seen_at=clock_timestamp()-interval '31 minutes' WHERE id=$1",
    )
    .bind(actor.session_id.into_uuid())
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        db.list_sessions(actor, None).await.unwrap(),
        [] as [domain::SessionId; 0]
    );
    assert_eq!(
        db.revoke_all(first).await,
        Err(domain::Error::Unauthenticated)
    );
}

#[tokio::test]
async fn revoke_all_includes_preceding_login_without_rewriting_prior_revocations() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
    let db = Database::new(pool.clone()).unwrap();
    let email = format!("{}@example.test", Uuid::now_v7());
    let (caller, token) = session(&db, &email).await;
    let (retired, old) = session(&db, &email).await;
    db.revoke(&old).await.unwrap();
    let record = revocation_record(&db, retired.session_id).await;
    assert_eq!(record.1, 1);
    assert_eq!(
        db.revoke_all(&old).await,
        Err(domain::Error::Unauthenticated)
    );

    let link = SystemSecrets.token();
    db.issue_login(&email, &link).await.unwrap();
    let mut barrier = pool.begin().await.unwrap();
    let blocker = sqlx::query_scalar("SELECT pg_backend_pid() FROM users WHERE id=$1 FOR UPDATE")
        .bind(caller.user_id.into_uuid())
        .fetch_one(&mut *barrier)
        .await
        .unwrap();
    let login_db = db.clone();
    let login =
        tokio::spawn(async move { login_db.consume_login(&link, &SystemSecrets.token()).await });
    let login_pid = waiting_session(&pool, blocker, 0).await;
    let revoke_db = db.clone();
    let revoke = tokio::spawn(async move { revoke_db.revoke_all(&token).await });
    waiting_session(&pool, blocker, login_pid).await;
    barrier.commit().await.unwrap();

    let added = login.await.unwrap().unwrap();
    revoke.await.unwrap().unwrap();
    assert_eq!(revocation_record(&db, added.session_id).await.1, 1);
    assert_eq!(revocation_record(&db, caller.session_id).await.1, 1);
    assert_eq!(revocation_record(&db, retired.session_id).await, record);
}

#[tokio::test]
async fn revoke_all_rechecks_token_and_identity_after_lock_waits() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
    let db = Database::new(pool.clone()).unwrap();
    let email = format!("{}@example.test", Uuid::now_v7());
    let (caller, old) = session(&db, &email).await;
    let (_, sibling) = session(&db, &email).await;
    let mut rotation = pool.begin().await.unwrap();
    let new = SystemSecrets.token();
    let blocker = sqlx::query_scalar(
        "UPDATE sessions SET token_hash=$1 WHERE id=$2 RETURNING pg_backend_pid()",
    )
    .bind(infrastructure::postgres::token_hash(&new))
    .bind(caller.session_id.into_uuid())
    .fetch_one(&mut *rotation)
    .await
    .unwrap();
    let revoke_db = db.clone();
    let revoke = tokio::spawn(async move { revoke_db.revoke_all(&old).await });
    waiting_session(&pool, blocker, 0).await;
    rotation.commit().await.unwrap();

    assert_eq!(revoke.await.unwrap(), Err(domain::Error::Unauthenticated));
    assert!(db.authenticate(&new).await.is_ok());
    assert!(db.authenticate(&sibling).await.is_ok());

    let mut disable = pool.begin().await.unwrap();
    let blocker =
        sqlx::query_scalar("UPDATE users SET disabled=true WHERE id=$1 RETURNING pg_backend_pid()")
            .bind(caller.user_id.into_uuid())
            .fetch_one(&mut *disable)
            .await
            .unwrap();
    let revoke_db = db.clone();
    let revoke = tokio::spawn(async move { revoke_db.revoke_all(&new).await });
    waiting_session(&pool, blocker, 0).await;
    disable.commit().await.unwrap();

    assert_eq!(revoke.await.unwrap(), Err(domain::Error::Unauthenticated));
    let retired: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sessions WHERE user_id=$1 AND revoked_at IS NOT NULL",
    )
    .bind(caller.user_id.into_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(retired, 0);
}
