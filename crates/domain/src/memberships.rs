use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Admin,
    Member,
    Viewer,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    ReadProject,
    WriteProject,
    ManageMembers,
    ManageOwners,
}
impl Role {
    #[must_use]
    pub fn allows(self, permission: Permission) -> bool {
        match permission {
            Permission::ReadProject => true,
            Permission::WriteProject => self != Self::Viewer,
            Permission::ManageMembers => matches!(self, Self::Owner | Self::Admin),
            Permission::ManageOwners => self == Self::Owner,
        }
    }
}
