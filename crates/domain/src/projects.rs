use crate::{Error, OrganizationId, ProjectId, validate_name};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub organization_id: OrganizationId,
    pub name: String,
    pub version: i64,
    pub archived: bool,
}
impl Project {
    /// Create an active project at version one.
    ///
    /// # Errors
    /// Returns `Invalid` when the name is empty, too long or contains control characters.
    pub fn new(
        id: ProjectId,
        organization_id: OrganizationId,
        name: String,
    ) -> Result<Self, Error> {
        validate_name(&name)?;
        Ok(Self {
            id,
            organization_id,
            name,
            version: 1,
            archived: false,
        })
    }
    /// Rename only at the expected version.
    ///
    /// # Errors
    /// Returns `Conflict` for a stale or exhausted version and `Invalid` for an invalid name.
    pub fn rename(&mut self, expected: i64, name: String) -> Result<(), Error> {
        if self.version != expected {
            return Err(Error::Conflict);
        }
        validate_name(&name)?;
        let next_version = self.version.checked_add(1).ok_or(Error::Conflict)?;
        self.name = name;
        self.version = next_version;
        Ok(())
    }
}
