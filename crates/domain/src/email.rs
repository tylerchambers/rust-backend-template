use crate::Error;

/// Normalized mailbox identity, restricted to ASCII unquoted mailboxes.
/// Whole-address lowercasing is product identity policy, not an RFC requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailAddress(String);

impl EmailAddress {
    /// Parse before allocating budgets, publishing credentials or enqueueing mail.
    ///
    /// # Errors
    /// Returns `Invalid` for malformed or unsupported mailbox syntax.
    pub fn parse(input: &str) -> Result<Self, Error> {
        let normalized = input.trim().to_ascii_lowercase();
        if normalized.len() > 254
            || !normalized.is_ascii()
            || normalized.contains('"')
            || normalized
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(Error::Invalid);
        }
        email_address::EmailAddress::parse_with_options(
            &normalized,
            email_address::Options::default()
                .without_display_text()
                .without_domain_literal(),
        )
        .map_err(|_| Error::Invalid)?;
        Ok(Self(normalized))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
