use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
async fn nonowner_role_denies_unscoped_and_cross_tenant_operations() {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::migrate(&pool).await.unwrap();
    let a = Uuid::now_v7();
    let b = Uuid::now_v7();
    for id in [a, b] {
        sqlx::query("INSERT INTO organizations(id,name) VALUES($1,'isolation')")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    let mut c = pool.acquire().await.unwrap();
    sqlx::query("SET ROLE template_app")
        .execute(&mut *c)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM organizations")
            .fetch_one(&mut *c)
            .await
            .unwrap(),
        0
    );
    sqlx::query("BEGIN").execute(&mut *c).await.unwrap();
    sqlx::query("SELECT set_config('app.organization_id',$1,true)")
        .bind(a.to_string())
        .execute(&mut *c)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM organizations")
            .fetch_one(&mut *c)
            .await
            .unwrap(),
        1
    );
    for q in [
        "UPDATE organizations SET name='attack' WHERE id=$1",
        "DELETE FROM organizations WHERE id=$1",
    ] {
        assert_eq!(
            sqlx::query(q)
                .bind(b)
                .execute(&mut *c)
                .await
                .unwrap()
                .rows_affected(),
            0
        );
    }
    assert!(
        sqlx::query("INSERT INTO projects(id,organization_id,name) VALUES($1,$2,'attack')")
            .bind(Uuid::now_v7())
            .bind(b)
            .execute(&mut *c)
            .await
            .is_err()
    );
    sqlx::query("ROLLBACK").execute(&mut *c).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM organizations")
            .fetch_one(&mut *c)
            .await
            .unwrap(),
        0
    );
    assert!(
        !sqlx::query_scalar::<_, bool>(
            "SELECT rolbypassrls OR rolsuper FROM pg_roles WHERE rolname=current_user"
        )
        .fetch_one(&mut *c)
        .await
        .unwrap()
    );
    sqlx::query("RESET ROLE").execute(&mut *c).await.unwrap();
}
