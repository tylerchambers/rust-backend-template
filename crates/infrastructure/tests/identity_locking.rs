#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Database regression assertions"
)]
use application::Actor;
use application::Organizations;
use application::Secrets;
use application::authentication::IdentityRepository;
use application::memberships::Memberships;
use domain::OrganizationId;
use infrastructure::postgres::Database;
use infrastructure::system::SystemIds;
use infrastructure::system::SystemSecrets;
use sqlx::{PgPool, Postgres, Transaction};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

// Trigger DDL requires a table lock; run these independent fixtures sequentially
// so teardown cannot wait on another test that is itself waiting for fixture DDL.
#[tokio::test]
async fn identity_locking_regressions() {
    tokio::time::timeout(Duration::from_secs(30), async {
        login_after_role_change_receives_a_live_session().await;
        shared_actors_in_opposite_organizations_use_one_identity_order().await;
        login_committed_before_change_is_included_in_retirement().await;
        authority_expired_during_identity_wait_cannot_publish().await;
        for remove_first in [false, true] {
            concurrent_admin_exits_preserve_last_admin(remove_first).await;
        }
    })
    .await
    .expect("bounded identity-locking regression matrix");
}

async fn authority_expired_during_identity_wait_cannot_publish() {
    let f = Fixture::new().await;
    let mut barrier = f.admin.begin().await.unwrap();
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(f.actors[1].user_id.into_uuid())
        .execute(&mut *barrier)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE sessions SET expires_at=clock_timestamp()+interval '2 seconds' WHERE id=$1",
    )
    .bind(f.actors[0].session_id.into_uuid())
    .execute(&f.admin)
    .await
    .unwrap();
    let blocker = pid(&mut barrier).await;
    let service = f.service();
    let admin = f.actors[0].clone();
    let org = f.org;
    let target = f.actors[1].user_id;
    let change = tokio::spawn(async move { service.change_role(&admin, org, target, true).await });
    blocked_by(&f.admin, blocker).await;
    // Wait on the database's actual authority deadline, not an arbitrary sleep.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let expired: bool = sqlx::query_scalar(
                "SELECT expires_at<=clock_timestamp() FROM sessions WHERE id=$1",
            )
            .bind(f.actors[0].session_id.into_uuid())
            .fetch_one(&f.admin)
            .await
            .unwrap();
            if expired {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    barrier.commit().await.unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), change)
            .await
            .unwrap()
            .unwrap(),
        Err(domain::Error::Forbidden)
    );
    let state: (bool, bool, i64) = sqlx::query_as(r"
        SELECT m.is_admin, s.revoked_at IS NULL,
          (SELECT count(*) FROM audit_events WHERE organization_id=$1 AND action='membership.role_changed')
        FROM memberships m JOIN sessions s ON s.user_id=m.user_id
        WHERE m.organization_id=$1 AND s.id=$2
        ")
        .bind(org.into_uuid()).bind(f.actors[1].session_id.into_uuid()).fetch_one(&f.admin).await.unwrap();
    assert_eq!(state, (false, true, 0));
}

struct Fixture {
    admin: PgPool,
    db: Arc<Database>,
    actors: Vec<Actor>,
    org: OrganizationId,
}
impl Fixture {
    async fn new() -> Self {
        let admin = PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        infrastructure::postgres::migrations::migrate(&admin)
            .await
            .unwrap();
        let runtime = PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();
        let db = Arc::new(Database::new(runtime).unwrap());
        let mut actors = Vec::new();
        for _ in 0..2 {
            let link = SystemSecrets.token();
            db.issue_login(&format!("{}@example.test", Uuid::now_v7()), &link)
                .await
                .unwrap();
            actors.push(
                db.consume_login(&link, &SystemSecrets.token())
                    .await
                    .unwrap(),
            );
        }
        let org = Organizations::new(db.clone(), Arc::new(SystemIds))
            .create(&actors[0], "Lock regression".into())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO memberships(organization_id,user_id,is_admin) VALUES($1,$2,false)",
        )
        .bind(org.into_uuid())
        .bind(actors[1].user_id.into_uuid())
        .execute(&admin)
        .await
        .unwrap();
        Self {
            admin,
            db,
            actors,
            org,
        }
    }
    fn service(&self) -> Memberships {
        Memberships::new(self.db.clone())
    }
}

// Observe a real wait rather than relying on scheduler timing.
async fn blocked_by(pool: &PgPool, blocker: i32) -> i32 {
    blocked_by_except(pool, blocker, 0).await
}
async fn blocked_by_except(pool: &PgPool, blocker: i32, excluded: i32) -> i32 {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let pid: Option<i32> = sqlx::query_scalar("SELECT pid FROM pg_stat_activity WHERE usename='template_app' AND pid<>$2 AND ($1=ANY(pg_blocking_pids(pid)) OR $2=ANY(pg_blocking_pids(pid))) LIMIT 1")
                .bind(blocker).bind(excluded).fetch_optional(pool).await.unwrap();
            if let Some(pid) = pid { return pid; }
            tokio::task::yield_now().await;
        }
    }).await.expect("second operation reached the database barrier")
}
async fn pid(tx: &mut Transaction<'_, Postgres>) -> i32 {
    sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
}

async fn login_after_role_change_receives_a_live_session() {
    let f = Fixture::new().await;
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id=$1")
        .bind(f.actors[1].user_id.into_uuid())
        .fetch_one(&f.admin)
        .await
        .unwrap();
    let link = SystemSecrets.token();
    f.db.issue_login(&email, &link).await.unwrap();
    // Pause the real application after membership UPDATE, before retirement.
    // Only this random organization's writes encounter the test-only trigger.
    let name = format!("lock_probe_{}", f.org.into_uuid().simple());
    let ddl = format!(
        "CREATE FUNCTION {name}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(7351); RETURN NEW; END $$; CREATE TRIGGER {name} AFTER UPDATE ON memberships FOR EACH ROW WHEN (NEW.organization_id='{}'::uuid) EXECUTE FUNCTION {name}()",
        f.org
    );
    sqlx::raw_sql(sqlx::AssertSqlSafe(ddl))
        .execute(&f.admin)
        .await
        .unwrap();
    let mut barrier = f.admin.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(7351)")
        .execute(&mut *barrier)
        .await
        .unwrap();
    let barrier_pid = pid(&mut barrier).await;
    let service = f.service();
    let admin = f.actors[0].clone();
    let target = f.actors[1].clone();
    let org = f.org;
    let changing =
        tokio::spawn(async move { service.change_role(&admin, org, target.user_id, true).await });
    let change_pid = blocked_by(&f.admin, barrier_pid).await;
    let db = f.db.clone();
    let new_token = SystemSecrets.token();
    let token = new_token.clone();
    let login = tokio::spawn(async move { db.consume_login(&link, &token).await });
    blocked_by(&f.admin, change_pid).await;
    barrier.commit().await.unwrap();
    let changed = tokio::time::timeout(Duration::from_secs(5), changing)
        .await
        .unwrap()
        .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP TRIGGER {name} ON memberships; DROP FUNCTION {name}()"
    )))
    .execute(&f.admin)
    .await
    .unwrap();
    assert_eq!(changed, Ok(()), "application role change must commit");
    let state: (bool, bool, i64) = sqlx::query_as("SELECT m.is_admin, s.revoked_at IS NOT NULL, (SELECT count(*) FROM audit_events WHERE organization_id=$1 AND action='membership.role_changed') FROM memberships m JOIN sessions s ON s.user_id=m.user_id WHERE m.organization_id=$1 AND s.id=$2")
        .bind(org.into_uuid()).bind(f.actors[1].session_id.into_uuid()).fetch_one(&f.admin).await.unwrap();
    assert_eq!(state, (true, true, 1));
    let signed_in = tokio::time::timeout(Duration::from_secs(5), login)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        f.db.authenticate(&new_token).await.unwrap().session_id,
        signed_in.session_id
    );
}

async fn shared_actors_in_opposite_organizations_use_one_identity_order() {
    let f = Fixture::new().await;
    let other = Organizations::new(f.db.clone(), Arc::new(SystemIds))
        .create(&f.actors[1], "Other tenant".into())
        .await
        .unwrap();
    sqlx::query("INSERT INTO memberships(organization_id,user_id,is_admin) VALUES($1,$2,false)")
        .bind(other.into_uuid())
        .bind(f.actors[0].user_id.into_uuid())
        .execute(&f.admin)
        .await
        .unwrap();
    assert!(f.actors[0].user_id.into_uuid() < f.actors[1].user_id.into_uuid());
    let mut barrier = f.admin.begin().await.unwrap();
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(f.actors[0].user_id.into_uuid())
        .execute(&mut *barrier)
        .await
        .unwrap();
    let blocker = pid(&mut barrier).await;
    let mut changes = Vec::new();
    // Reverse actor order is deliberate: an actor-first implementation locks
    // the high identity first here and can deadlock with the other tenant.
    for i in [1, 0] {
        let service = f.service();
        let actor = f.actors[i].clone();
        let target = f.actors[1 - i].user_id;
        let org = if i == 0 { f.org } else { other };
        changes.push(tokio::spawn(async move {
            service.change_role(&actor, org, target, true).await
        }));
        if i == 1 {
            blocked_by(&f.admin, blocker).await;
        }
    }
    let first = blocked_by(&f.admin, blocker).await;
    blocked_by_except(&f.admin, blocker, first).await;
    // No global gate: an unrelated identity/tenant progresses while both wait.
    let independent = Fixture::new().await;
    tokio::time::timeout(
        Duration::from_secs(5),
        independent.service().change_role(
            &independent.actors[0],
            independent.org,
            independent.actors[1].user_id,
            true,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    barrier.commit().await.unwrap();
    let mut succeeded = 0;
    for change in changes {
        let result = tokio::time::timeout(Duration::from_secs(5), change)
            .await
            .unwrap()
            .unwrap();
        match result {
            Ok(()) => succeeded += 1,
            Err(domain::Error::Unauthenticated | domain::Error::Forbidden) => {}
            other => panic!("unexpected cross-tenant result: {other:?}"),
        }
    }
    assert_eq!(
        succeeded, 1,
        "winner retires the other caller before it is admitted"
    );
    let audits: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE organization_id=ANY($1) AND action='membership.role_changed'")
        .bind(vec![f.org.into_uuid(), other.into_uuid()]).fetch_one(&f.admin).await.unwrap();
    assert_eq!(audits, 1);
}

async fn login_committed_before_change_is_included_in_retirement() {
    let f = Fixture::new().await;
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id=$1")
        .bind(f.actors[1].user_id.into_uuid())
        .fetch_one(&f.admin)
        .await
        .unwrap();
    let link = SystemSecrets.token();
    f.db.issue_login(&email, &link).await.unwrap();
    let mut barrier = f.admin.begin().await.unwrap();
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(f.actors[1].user_id.into_uuid())
        .execute(&mut *barrier)
        .await
        .unwrap();
    let blocker = pid(&mut barrier).await;
    let db = f.db.clone();
    let token = SystemSecrets.token();
    let login_token = token.clone();
    let login = tokio::spawn(async move { db.consume_login(&link, &login_token).await });
    let login_pid = blocked_by(&f.admin, blocker).await;
    let service = f.service();
    let admin = f.actors[0].clone();
    let target = f.actors[1].user_id;
    let org = f.org;
    let change = tokio::spawn(async move { service.change_role(&admin, org, target, true).await });
    blocked_by_except(&f.admin, blocker, login_pid).await;
    barrier.commit().await.unwrap();
    let logged_in = tokio::time::timeout(Duration::from_secs(5), login)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), change)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(logged_in.user_id, target);
    assert!(matches!(
        f.db.authenticate(&token).await,
        Err(domain::Error::Unauthenticated)
    ));
    let live: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sessions WHERE user_id=$1 AND revoked_at IS NULL")
            .bind(target.into_uuid())
            .fetch_one(&f.admin)
            .await
            .unwrap();
    assert_eq!(live, 0);
}

async fn concurrent_admin_exits_preserve_last_admin(remove_first: bool) {
    let f = Fixture::new().await;
    let org = f.org;
    sqlx::query("UPDATE memberships SET is_admin=true WHERE organization_id=$1 AND user_id=$2")
        .bind(org.into_uuid())
        .bind(f.actors[1].user_id.into_uuid())
        .execute(&f.admin)
        .await
        .unwrap();

    // Hold the first caller at its identity lock. The other admin uses a
    // different identity, but cannot check the admin count until the first commits.
    let mut barrier = f.admin.begin().await.unwrap();
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(f.actors[0].user_id.into_uuid())
        .execute(&mut *barrier)
        .await
        .unwrap();
    let blocker = pid(&mut barrier).await;
    let first = f.actors[0].clone();
    let service = f.service();
    let first_exit = tokio::spawn(async move {
        if remove_first {
            service.remove(&first, org, first.user_id).await
        } else {
            service.change_role(&first, org, first.user_id, false).await
        }
    });
    let first_pid = blocked_by(&f.admin, blocker).await;
    let second = f.actors[1].clone();
    let service = f.service();
    let second_exit = tokio::spawn(async move {
        if remove_first {
            service
                .change_role(&second, org, second.user_id, false)
                .await
        } else {
            service.remove(&second, org, second.user_id).await
        }
    });
    blocked_by(&f.admin, first_pid).await;
    barrier.commit().await.unwrap();
    assert_eq!(first_exit.await.unwrap(), Ok(()));
    assert_eq!(second_exit.await.unwrap(), Err(domain::Error::Conflict));

    let admins: Vec<Uuid> =
        sqlx::query_scalar("SELECT user_id FROM memberships WHERE organization_id=$1 AND is_admin")
            .bind(org.into_uuid())
            .fetch_all(&f.admin)
            .await
            .unwrap();
    assert_eq!(admins, [f.actors[1].user_id.into_uuid()]);
    for (actor, expected_live) in [(&f.actors[0], remove_first), (&f.actors[1], true)] {
        let live: bool = sqlx::query_scalar("SELECT revoked_at IS NULL FROM sessions WHERE id=$1")
            .bind(actor.session_id.into_uuid())
            .fetch_one(&f.admin)
            .await
            .unwrap();
        assert_eq!(live, expected_live);
    }
}
