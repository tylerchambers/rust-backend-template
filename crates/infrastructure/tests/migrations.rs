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
    infrastructure::migrate(&pool).await.unwrap();
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
