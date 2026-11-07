use domain::{OrganizationId, Project, ProjectId};
use uuid::Uuid;

#[test]
fn project_identity_is_typed_without_changing_uuid_serialization() {
    let id = ProjectId::from(Uuid::now_v7());
    let organization = OrganizationId::from(Uuid::now_v7());
    let project = Project::new(id, organization, "typed identity".into()).unwrap();
    assert_eq!(project.id, id);
    assert_eq!(project.organization_id, organization);
    assert_eq!(id.into_uuid().to_string(), id.to_string());
}
