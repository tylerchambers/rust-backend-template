/// Application-owned identity generation; callers can inject deterministic IDs.
pub trait IdGenerator: Send + Sync {
    fn organization(&self) -> domain::OrganizationId;
    fn project(&self) -> domain::ProjectId;
    fn invitation(&self) -> domain::InvitationId;
}
