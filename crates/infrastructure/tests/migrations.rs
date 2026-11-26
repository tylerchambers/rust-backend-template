#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use std::{fs, path::PathBuf};
#[tokio::test]
async fn sqlx_rejects_modified_applied_migration() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    let temp = PathBuf::from(std::env::var("TMPDIR").expect("scratch directory required"))
        .join(format!("rbt-migrations-{}", uuid::Uuid::now_v7()));
    fs::create_dir(&temp).unwrap();
    async {
        for entry in fs::read_dir(root).unwrap() {
            let entry = entry.unwrap();
            fs::copy(entry.path(), temp.join(entry.file_name())).unwrap();
        }
        let first = temp.join("202611070001_organizations.sql");
        let source = fs::read_to_string(&first).unwrap();
        fs::write(first, format!("{source}\n-- unauthorized rewrite\n")).unwrap();
        let migrator = sqlx::migrate::Migrator::new(temp.as_path()).await.unwrap();
        assert!(matches!(
            migrator.run(&pool).await,
            Err(sqlx::migrate::MigrateError::VersionMismatch(
                202_611_070_001
            ))
        ));
    }
    .await;
    fs::remove_dir_all(temp).unwrap();
}

#[tokio::test]
async fn admin_cutover_preserves_privileges_on_memberships_and_invitations() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    // Exercise the actual forward migration without addressing any live tenant tables.
    sqlx::raw_sql(
        "SET LOCAL search_path TO pg_temp;
         CREATE TEMP TABLE memberships (
             id integer PRIMARY KEY,
             role text NOT NULL CHECK (role IN ('owner','admin','member','viewer'))
         );
         CREATE TEMP TABLE invitations (
             id integer PRIMARY KEY,
             role text NOT NULL CHECK (role IN ('owner','admin','member','viewer'))
         );
         CREATE TEMP TABLE reauthentication_proofs (id integer PRIMARY KEY);
         INSERT INTO memberships VALUES (1,'owner'), (2,'admin'), (3,'member'), (4,'viewer');
         INSERT INTO invitations SELECT * FROM memberships;",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../migrations/202611070011_admin_membership.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();

    let access: Vec<(i32, bool, bool)> = sqlx::query_as(
        "SELECT id,m.is_admin,i.is_admin
         FROM memberships m JOIN invitations i USING (id) ORDER BY id",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    assert_eq!(
        access,
        [
            (1, true, true),
            (2, true, true),
            (3, false, false),
            (4, false, false)
        ]
    );
    let removed: bool =
        sqlx::query_scalar("SELECT to_regclass('pg_temp.reauthentication_proofs') IS NULL")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert!(removed);
    tx.rollback().await.unwrap();
}
