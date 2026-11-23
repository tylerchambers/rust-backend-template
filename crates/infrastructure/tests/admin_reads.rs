#![allow(
    clippy::unwrap_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::administration::{Administration, InvitationStatus};
use application::authentication::IdentityRepository;
use application::{Actor, Secrets};
use domain::{Error, OrganizationId, Role};
use infrastructure::postgres::{Database, migrations};
use infrastructure::system::SystemSecrets;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

struct Tenant {
    org: OrganizationId,
    users: Vec<Uuid>,
    invitations: Vec<Uuid>,
    events: Vec<Uuid>,
}

// Alternate tenants in UUID order, independently for each cursor namespace.
fn interleaved_ids() -> (Vec<Uuid>, Vec<Uuid>) {
    let mut ids: Vec<_> = (0..102).map(|_| Uuid::now_v7()).collect();
    ids.sort_unstable();
    ids.as_chunks::<2>().0.iter().map(|&[a, b]| (a, b)).unzip()
}

fn tenant_pair() -> [Tenant; 2] {
    let (users_a, users_b) = interleaved_ids();
    let (invites_a, invites_b) = interleaved_ids();
    let (events_a, events_b) = interleaved_ids();
    [
        Tenant {
            org: Uuid::now_v7().into(),
            users: users_a,
            invitations: invites_a,
            events: events_a,
        },
        Tenant {
            org: Uuid::now_v7().into(),
            users: users_b,
            invitations: invites_b,
            events: events_b,
        },
    ]
}

async fn seed(pool: &PgPool, tenant: &Tenant) {
    let mut tx = pool.begin().await.unwrap();
    let org = tenant.org.into_uuid();
    sqlx::query("INSERT INTO organizations(id,name) VALUES($1,'Admin reads')")
        .bind(org)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO users(id,email,disabled)
         SELECT id,id::text || '@example.test',n=51
         FROM unnest($1::uuid[]) WITH ORDINALITY AS seed(id,n)",
    )
    .bind(&tenant.users)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO memberships(organization_id,user_id,role)
         SELECT $1,id,CASE n WHEN 1 THEN 'owner' WHEN 2 THEN 'admin'
                             WHEN 4 THEN 'viewer' ELSE 'member' END
         FROM unnest($2::uuid[]) WITH ORDINALITY AS seed(id,n) ORDER BY n DESC",
    )
    .bind(org)
    .bind(&tenant.users)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO invitations(id,organization_id,email,role,token_hash,expires_at,accepted_at,revoked_at)
         SELECT id,$1,id::text || '@example.test','viewer',sha256(uuid_send(id)),
                CASE WHEN n<=3 THEN now()-interval '1 day' ELSE now()+interval '1 day' END,
                CASE WHEN n=1 THEN now()-interval '2 days' END,
                CASE WHEN n=2 THEN now()-interval '2 days' END
         FROM unnest($2::uuid[]) WITH ORDINALITY AS seed(id,n) ORDER BY n DESC",
    )
    .bind(org)
    .bind(&tenant.invitations)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO audit_events(id,organization_id,actor_id,action,subject,occurred_at,correlation_id)
         SELECT id,$1,$2,'membership.role_changed',subject,TIMESTAMPTZ 'epoch',
                CASE WHEN n>1 THEN id END
         FROM unnest($3::uuid[],$4::uuid[]) WITH ORDINALITY AS seed(id,subject,n) ORDER BY n DESC",
    )
    .bind(org)
    .bind(tenant.users[0])
    .bind(&tenant.events)
    .bind(&tenant.users)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

async fn login(db: &Database, mailbox: Uuid) -> Actor {
    let link = SystemSecrets.token();
    db.issue_login(&format!("{mailbox}@example.test"), &link)
        .await
        .unwrap();
    db.consume_login(&link, &SystemSecrets.token())
        .await
        .unwrap()
}

// Tenant-local only: other tests can write unrelated rows, and authentication can touch sessions.
async fn snapshot(pool: &PgPool, org: OrganizationId) -> (i64, i64, serde_json::Value) {
    sqlx::query_as(
        "SELECT (SELECT count(*) FROM memberships WHERE organization_id=$1),
                (SELECT count(*) FROM audit_events WHERE organization_id=$1),
                (SELECT jsonb_agg(to_jsonb(i) ORDER BY id) FROM invitations i WHERE organization_id=$1)",
    )
    .bind(org.into_uuid())
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn assert_pages<T>(
    expected: &[Uuid],
    foreign_cursor: Uuid,
    fetch: impl AsyncFn(Option<Uuid>) -> Vec<T>,
    key: impl Fn(&T) -> Uuid,
) -> Vec<T> {
    let mut rows = fetch(None).await;
    assert_eq!(rows.len(), 50);
    let last = fetch(rows.last().map(&key)).await;
    assert_eq!(last.len(), 1);
    assert!(fetch(last.last().map(&key)).await.is_empty());
    rows.extend(last);
    assert_eq!(rows.iter().map(&key).collect::<Vec<_>>(), expected);

    // The foreign key falls strictly between the 26th and 27th in-tenant rows.
    let suffix = fetch(Some(foreign_cursor)).await;
    assert_eq!(suffix.iter().map(key).collect::<Vec<_>>(), expected[26..]);
    rows
}

async fn assert_authorized(
    service: &Administration,
    actor: &Actor,
    local: &Tenant,
    other: &Tenant,
) {
    let invitations = assert_pages(
        &local.invitations,
        other.invitations[25],
        async |after| {
            service
                .invitations(actor, local.org, after.map(Into::into))
                .await
                .unwrap()
        },
        |row| row.id.into_uuid(),
    )
    .await;
    for (i, row) in invitations.iter().enumerate() {
        assert_eq!(row.email, format!("{}@example.test", local.invitations[i]));
        assert_eq!(row.role, Role::Viewer);
        assert!(matches!(
            (i, &row.status),
            (0, InvitationStatus::Accepted)
                | (1, InvitationStatus::Revoked)
                | (2, InvitationStatus::Expired)
                | (3.., InvitationStatus::Pending)
        ));
        assert_eq!(row.expires_at < chrono::Utc::now(), i < 3);
    }

    let events = assert_pages(
        &local.events,
        other.events[25],
        async |after| service.audit_events(actor, local.org, after).await.unwrap(),
        |row| row.id,
    )
    .await;
    for (i, row) in events.iter().enumerate() {
        assert_eq!(row.actor_id.into_uuid(), local.users[0]);
        assert_eq!(row.action, "membership.role_changed");
        assert_eq!(row.subject, local.users[i]);
        assert_eq!(row.occurred_at, chrono::DateTime::UNIX_EPOCH);
    }

    let directory = assert_pages(
        &local.users,
        other.users[25],
        async |after| {
            service
                .member_directory(actor, local.org, after.map(Into::into))
                .await
                .unwrap()
        },
        |row| row.user_id.into_uuid(),
    )
    .await;
    for (i, row) in directory.iter().enumerate() {
        assert_eq!(row.email, format!("{}@example.test", local.users[i]));
        let role = match i {
            0 => Role::Owner,
            1 => Role::Admin,
            3 => Role::Viewer,
            _ => Role::Member,
        };
        assert_eq!(row.role, role);
    }
}

async fn assert_forbidden(service: &Administration, actor: &Actor, org: OrganizationId) {
    assert!(matches!(
        service.invitations(actor, org, None).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service.audit_events(actor, org, None).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service.member_directory(actor, org, None).await,
        Err(Error::Forbidden)
    ));
}

#[tokio::test]
async fn admin_reads_enforce_tenant_authority_and_page_without_mutating_history() {
    let setup = PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    migrations::migrate(&setup).await.unwrap();
    let [local, other] = tenant_pair();
    seed(&setup, &local).await;
    seed(&setup, &other).await;

    let runtime = PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let role: String = sqlx::query_scalar("SELECT current_user::text")
        .fetch_one(&runtime)
        .await
        .unwrap();
    assert_eq!(role, "template_app");
    let db = Arc::new(Database::new(runtime).unwrap());
    let service = Administration::new(db.clone());
    let owner = login(&db, local.users[0]).await;
    let admin = login(&db, local.users[1]).await;
    let member = login(&db, local.users[2]).await;
    let viewer = login(&db, local.users[3]).await;
    let nonmember = login(&db, Uuid::now_v7()).await;

    let before = snapshot(&setup, local.org).await;
    for actor in [&owner, &admin] {
        assert_authorized(&service, actor, &local, &other).await;
    }
    for actor in [&member, &viewer, &nonmember] {
        assert_forbidden(&service, actor, local.org).await;
    }
    assert_forbidden(&service, &admin, other.org).await;
    assert_forbidden(&service, &admin, Uuid::now_v7().into()).await;

    // The actor still has the admin membership, but no longer has a live session.
    sqlx::query("UPDATE sessions SET revoked_at=now() WHERE id=$1")
        .bind(admin.session_id.into_uuid())
        .execute(&setup)
        .await
        .unwrap();
    assert_forbidden(&service, &admin, local.org).await;
    assert_eq!(snapshot(&setup, local.org).await, before);
}
