#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use domain::{Error, Project};
use uuid::Uuid;

#[test]
fn rejected_rename_preserves_every_project_field() {
    for (version, expected, name, error) in [
        (1, 0, "After", Error::Conflict),
        (1, 1, " ", Error::Invalid),
        (i64::MAX, i64::MAX, "After", Error::Conflict),
    ] {
        let mut project = Project {
            id: Uuid::now_v7().into(),
            organization_id: Uuid::now_v7().into(),
            name: "Before".into(),
            version,
            archived: true,
        };
        let before = project.clone();
        assert_eq!(project.rename(expected, name.into()), Err(error));
        assert_eq!(project.id, before.id);
        assert_eq!(project.organization_id, before.organization_id);
        assert_eq!(project.name, before.name);
        assert_eq!(project.version, before.version);
        assert_eq!(project.archived, before.archived);
    }
}

#[test]
fn project_rejects_invalid_names_and_stale_revisions() {
    let org = Uuid::now_v7();
    let id = Uuid::now_v7();
    assert!(Project::new(id.into(), org.into(), " ".into()).is_err());
    let mut p = Project::new(id.into(), org.into(), "Initial".into()).unwrap();
    assert_eq!(p.rename(0, "Next".into()), Err(Error::Conflict));
    assert_eq!(p.version, 1);
    p.rename(1, "Next".into()).unwrap();
    assert_eq!((p.name.as_str(), p.version), ("Next", 2));
}
