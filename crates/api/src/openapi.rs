use crate::{
    audit_events,
    authentication::{self, IdentityResponse, LoginConfirmationRequest, LoginRequest},
    error::ErrorBody,
    invitations, memberships,
    organizations::{self, CreateOrganizationRequest, OrganizationResponse},
    projects::{self, CreateProjectRequest, ProjectResponse, RenameProjectRequest},
    sessions,
};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        authentication::request_login,
        authentication::confirm,
        authentication::confirmation,
        authentication::logout,
        authentication::get_me,
        organizations::create_org,
        organizations::list_organizations,
        audit_events::list_audit_events,
        memberships::list_members,
        memberships::list_member_directory,
        projects::create_project,
        projects::list_projects,
        projects::get_project,
        projects::rename_project,
        invitations::revoke_invitation,
        memberships::remove_member,
        invitations::list_invitations,
        invitations::invite,
        invitations::accept,
        memberships::change_role,
        sessions::sessions,
        sessions::rotate,
        sessions::revoke_all,
        sessions::revoke_session,
        invitations::invitation_confirmation
    ),
    modifiers(&ContractSecurity),
    components(schemas(
        LoginRequest,
        LoginConfirmationRequest,
        CreateOrganizationRequest,
        CreateProjectRequest,
        RenameProjectRequest,
        IdentityResponse,
        OrganizationResponse,
        ProjectResponse,
        ErrorBody
    ))
)]
pub struct Contract;
struct ContractSecurity;
impl utoipa::Modify for ContractSecurity {
    fn modify(&self, contract: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
        if let Some(components) = &mut contract.components {
            components.add_security_scheme(
                "session",
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("__Host-session"))),
            );
        }
    }
}
#[must_use]
/// Serialize the statically derived API contract.
///
/// # Panics
/// Panics if the derived contract cannot be serialized, indicating a schema defect.
#[expect(
    clippy::expect_used,
    reason = "Serialization failure of the static contract is a programmer defect"
)]
pub fn openapi() -> String {
    Contract::openapi()
        .to_pretty_json()
        .expect("serializable contract")
}
