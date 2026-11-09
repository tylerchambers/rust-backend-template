#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Database regression assertions"
)]
use application::{
    Actor, IdentityRepository, Organizations, Secrets,
    memberships::{Memberships, ReauthenticationRepository},
};
use domain::{OrganizationId, Role};
use infrastructure::{
    Database,
    auth::{SystemClock, SystemIds, SystemSecrets},
};
use sqlx::{PgPool, Postgres, Transaction};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

// Trigger DDL requires a table lock; run these independent fixtures sequentially
// so teardown cannot wait on another test that is itself waiting for fixture DDL.
#[tokio::test]
async fn identity_locking_regressions() {
    tokio::time::timeout(Duration::from_secs(30), async {
        application_change_and_proof_are_atomic_without_deadlock().await;
        shared_actors_in_opposite_organizations_use_one_identity_order().await;
        login_committed_before_change_is_included_in_retirement().await;
        proof_first_does_not_cycle_on_organization_foreign_key().await;
        authority_expired_during_identity_wait_cannot_publish().await;
    })
    .await
    .expect("bounded identity-locking regression matrix");
}

async fn authority_expired_during_identity_wait_cannot_publish() {
    let f = Fixture::new().await;
    let proof = f.proof(0, 1, f.org).await;
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
    let owner = f.actors[0].clone();
    let org = f.org;
    let target = f.actors[1].user_id;
    let change = tokio::spawn(async move {
        service
            .change_role(&owner, org, target, Role::Viewer, &proof)
            .await
    });
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
    let state: (String, bool, i64, bool) = sqlx::query_as(r"
        SELECT m.role, s.revoked_at IS NULL,
          (SELECT count(*) FROM audit_events WHERE organization_id=$1 AND action='membership.role_changed'),
          (SELECT consumed_at IS NULL FROM reauthentication_proofs WHERE organization_id=$1)
        FROM memberships m JOIN sessions s ON s.user_id=m.user_id
        WHERE m.organization_id=$1 AND s.id=$2
        ")
        .bind(org.into_uuid()).bind(f.actors[1].session_id.into_uuid()).fetch_one(&f.admin).await.unwrap();
    assert_eq!(state, ("member".into(), true, 0, true));
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
        infrastructure::migrate(&admin).await.unwrap();
        let runtime =
            PgPool::connect("postgres://template_app:local-app-test@127.0.0.1:55439/template")
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
        sqlx::query("INSERT INTO memberships(organization_id,user_id,role) VALUES($1,$2,'member')")
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
        Memberships::new(self.db.clone(), Arc::new(SystemIds), Arc::new(SystemClock))
    }
    async fn proof(&self, actor: usize, target: usize, org: OrganizationId) -> String {
        let proof = SystemSecrets.token();
        self.db
            .issue_proof(
                &self.actors[actor],
                "change_role",
                self.actors[target].user_id.into_uuid(),
                org,
                &proof,
            )
            .await
            .unwrap();
        proof
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

async fn application_change_and_proof_are_atomic_without_deadlock() {
    let f = Fixture::new().await;
    let proof = f.proof(0, 1, f.org).await;
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
    let owner = f.actors[0].clone();
    let target = f.actors[1].clone();
    let org = f.org;
    let changing = tokio::spawn(async move {
        service
            .change_role(&owner, org, target.user_id, Role::Viewer, &proof)
            .await
    });
    let change_pid = blocked_by(&f.admin, barrier_pid).await;
    let db = f.db.clone();
    let target = f.actors[1].clone();
    let issuing = tokio::spawn(async move {
        db.issue_proof(
            &target,
            "invite",
            org.into_uuid(),
            org,
            &SystemSecrets.token(),
        )
        .await
    });
    let proof_pid = blocked_by(&f.admin, change_pid).await;
    let db = f.db.clone();
    let new_token = SystemSecrets.token();
    let token = new_token.clone();
    let login = tokio::spawn(async move { db.consume_login(&link, &token).await });
    blocked_by_except(&f.admin, change_pid, proof_pid).await;
    barrier.commit().await.unwrap();
    let changed = tokio::time::timeout(Duration::from_secs(5), changing)
        .await
        .unwrap()
        .unwrap();
    let issued = tokio::time::timeout(Duration::from_secs(5), issuing)
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
    assert!(
        matches!(
            issued,
            Err(domain::Error::Forbidden | domain::Error::Unauthenticated)
        ),
        "retired session cannot issue proof: {issued:?}"
    );
    let state: (String, bool, i64) = sqlx::query_as("SELECT m.role, s.revoked_at IS NOT NULL, (SELECT count(*) FROM audit_events WHERE organization_id=$1 AND action='membership.role_changed') FROM memberships m JOIN sessions s ON s.user_id=m.user_id WHERE m.organization_id=$1 AND s.id=$2")
        .bind(org.into_uuid()).bind(f.actors[1].session_id.into_uuid()).fetch_one(&f.admin).await.unwrap();
    assert_eq!(state, ("viewer".into(), true, 1));
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
    sqlx::query("INSERT INTO memberships(organization_id,user_id,role) VALUES($1,$2,'member')")
        .bind(other.into_uuid())
        .bind(f.actors[0].user_id.into_uuid())
        .execute(&f.admin)
        .await
        .unwrap();
    assert!(f.actors[0].user_id.into_uuid() < f.actors[1].user_id.into_uuid());
    let proofs = [f.proof(0, 1, f.org).await, f.proof(1, 0, other).await];
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
        let proof = proofs[i].clone();
        changes.push(tokio::spawn(async move {
            service
                .change_role(&actor, org, target, Role::Viewer, &proof)
                .await
        }));
        if i == 1 {
            blocked_by(&f.admin, blocker).await;
        }
    }
    let first = blocked_by(&f.admin, blocker).await;
    blocked_by_except(&f.admin, blocker, first).await;
    // No global gate: an unrelated identity/tenant progresses while both wait.
    let independent = Fixture::new().await;
    let proof = independent.proof(0, 1, independent.org).await;
    tokio::time::timeout(
        Duration::from_secs(5),
        independent.service().change_role(
            &independent.actors[0],
            independent.org,
            independent.actors[1].user_id,
            Role::Viewer,
            &proof,
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
    let proof = f.proof(0, 1, f.org).await;
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
    let owner = f.actors[0].clone();
    let target = f.actors[1].user_id;
    let org = f.org;
    let change = tokio::spawn(async move {
        service
            .change_role(&owner, org, target, Role::Viewer, &proof)
            .await
    });
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

async fn proof_first_does_not_cycle_on_organization_foreign_key() {
    let f = Fixture::new().await;
    let proof = f.proof(0, 1, f.org).await;
    let mut barrier = f.admin.begin().await.unwrap();
    sqlx::query("SELECT id FROM sessions WHERE id=$1 FOR UPDATE")
        .bind(f.actors[1].session_id.into_uuid())
        .execute(&mut *barrier)
        .await
        .unwrap();
    let blocker = pid(&mut barrier).await;
    let db = f.db.clone();
    let actor = f.actors[1].clone();
    let org = f.org;
    let issuing = tokio::spawn(async move {
        db.issue_proof(
            &actor,
            "invite",
            org.into_uuid(),
            org,
            &SystemSecrets.token(),
        )
        .await
    });
    let proof_pid = blocked_by(&f.admin, blocker).await;
    let service = f.service();
    let owner = f.actors[0].clone();
    let target = f.actors[1].user_id;
    let changing = tokio::spawn(async move {
        service
            .change_role(&owner, org, target, Role::Viewer, &proof)
            .await
    });
    blocked_by(&f.admin, proof_pid).await;
    barrier.commit().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(5), issuing)
            .await
            .unwrap()
            .unwrap()
            .is_ok()
    );
    tokio::time::timeout(Duration::from_secs(5), changing)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
