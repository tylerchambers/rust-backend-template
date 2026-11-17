use crate::Error;

/// Validate the shared organization/project name contract.
///
/// # Errors
/// Returns `Invalid` for blank names, control characters or more than 120 characters.
pub fn validate_name(name: &str) -> Result<(), Error> {
    if name.trim().is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
        Err(Error::Invalid)
    } else {
        Ok(())
    }
}
