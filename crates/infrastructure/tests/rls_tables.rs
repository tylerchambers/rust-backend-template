#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use sqlx::PgPool;
use uuid::Uuid;

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "One ordered integration journey retains shared state and security assertions"
)]
async fn every_tenant_table_hides_other_tenants_and_discovery_is_read_only_local() {
    let pool = PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
    let user = Uuid::now_v7();
    let other_user = Uuid::now_v7();
    for u in [user, other_user] {
        sqlx::query("INSERT INTO users(id,email) VALUES($1,$2)")
            .bind(u)
            .bind(format!("{u}@example.test"))
            .execute(&pool)
            .await
            .unwrap();
    }
    let a = Uuid::now_v7();
    let b = Uuid::now_v7();
    for (org, u) in [(a, user), (b, other_user)] {
        sqlx::query("INSERT INTO organizations(id,name) VALUES($1,'RLS')")
            .bind(org)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO memberships(organization_id,user_id,is_admin) VALUES($1,$2,true)")
            .bind(org)
            .bind(u)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO projects(id,organization_id,name) VALUES($1,$1,'RLS')")
            .bind(org)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO project_index(project_id,organization_id,name,version) VALUES($1,$1,'RLS',1)").bind(org).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO audit_events(id,organization_id,actor_id,action,subject) VALUES($1,$1,$2,'test',$1)").bind(org).bind(u).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO invitations(id,organization_id,email,is_admin,token_hash,expires_at) VALUES($1,$1,'rls@example.test',false,$2,clock_timestamp()+interval '1 hour')").bind(org).bind(infrastructure::postgres::token_hash(&org.to_string())).execute(&pool).await.unwrap();
    }
    let tables = [
        ("organizations", "id"),
        ("memberships", "organization_id"),
        ("projects", "organization_id"),
        ("project_index", "organization_id"),
        ("invitations", "organization_id"),
        ("audit_events", "organization_id"),
    ];
    let mut c = pool.acquire().await.unwrap();
    sqlx::query("SET ROLE template_app")
        .execute(&mut *c)
        .await
        .unwrap();
    for (table, _) in tables {
        let n: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                .fetch_one(&mut *c)
                .await
                .unwrap();
        assert_eq!(n, 0, "unscoped {table}");
    }
    sqlx::query("BEGIN").execute(&mut *c).await.unwrap();
    sqlx::query("SELECT set_config('app.organization_id',$1,true)")
        .bind(a.to_string())
        .execute(&mut *c)
        .await
        .unwrap();
    for (table, column) in tables {
        let n: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FROM {table} WHERE {column}=$1"
        )))
        .bind(b)
        .fetch_one(&mut *c)
        .await
        .unwrap();
        assert_eq!(n, 0, "cross-tenant {table}");
        let n: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FROM {table} WHERE {column}=$1"
        )))
        .bind(a)
        .fetch_one(&mut *c)
        .await
        .unwrap();
        assert!(n >= 1, "own {table}");
    }
    sqlx::query("ROLLBACK").execute(&mut *c).await.unwrap();
    sqlx::query("BEGIN").execute(&mut *c).await.unwrap();
    sqlx::query("SELECT set_config('app.discovery_user_id',$1,true)")
        .bind(user.to_string())
        .execute(&mut *c)
        .await
        .unwrap();
    let visible: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM organizations")
        .fetch_all(&mut *c)
        .await
        .unwrap();
    assert_eq!(visible, vec![a]);
    let changed = sqlx::query("UPDATE organizations SET name='forbidden' WHERE id=$1")
        .bind(a)
        .execute(&mut *c)
        .await
        .unwrap()
        .rows_affected();
    assert_eq!(changed, 0);
    let visible: Vec<Uuid> = sqlx::query_scalar("SELECT user_id FROM memberships")
        .fetch_all(&mut *c)
        .await
        .unwrap();
    assert_eq!(visible, vec![user]);
    sqlx::query("COMMIT").execute(&mut *c).await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM organizations")
        .fetch_one(&mut *c)
        .await
        .unwrap();
    assert_eq!(n, 0, "discovery context leaked from transaction");
    sqlx::query("RESET ROLE").execute(&mut *c).await.unwrap();
}
