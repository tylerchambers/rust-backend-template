use domain::{Error, Project};
use uuid::Uuid;
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
