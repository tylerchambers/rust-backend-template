use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! identifier {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);
        impl $name {
            pub const fn into_uuid(self) -> Uuid {
                self.0
            }
        }
        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}
identifier!(UserId);
identifier!(SessionId);
identifier!(OrganizationId);
identifier!(ProjectId);
identifier!(InvitationId);

/// Tenant and resource identities are not interchangeable.
/// ```compile_fail
/// use domain::{OrganizationId, ProjectId};
/// fn project(_: ProjectId) {}
/// let organization = OrganizationId::from(uuid::Uuid::nil());
/// project(organization);
/// ```
const _: () = ();
