/// Closed operational categories; dependency messages can contain credentials or
/// SQL values, so neither Display nor Debug of an upstream error is emitted.
#[derive(Debug, PartialEq, Eq)]
enum Failure {
    Connection,
    PoolClosed,
    Timeout,
    Permission,
    Conflict,
    Constraint,
    Other,
}
impl Failure {
    fn label(&self) -> &'static str {
        match self {
            Self::Connection => "connection",
            Self::PoolClosed => "pool_closed",
            Self::Timeout => "timeout",
            Self::Permission => "permission",
            Self::Conflict => "transaction_conflict",
            Self::Constraint => "constraint",
            Self::Other => "other",
        }
    }
    fn classify(error: &sqlx::Error) -> Self {
        match error {
            sqlx::Error::PoolClosed => Self::PoolClosed,
            sqlx::Error::PoolTimedOut => Self::Timeout,
            sqlx::Error::Io(_) | sqlx::Error::Tls(_) => Self::Connection,
            sqlx::Error::Database(db) => match db.code().as_deref() {
                Some("42501" | "28000" | "28P01") => Self::Permission,
                Some("40001" | "40P01") => Self::Conflict,
                Some("57014" | "55P03") => Self::Timeout,
                Some(code) if code.starts_with("23") => Self::Constraint,
                Some(code) if code.starts_with("08") => Self::Connection,
                _ => Self::Other,
            },
            _ => Self::Other,
        }
    }
}

/// The adapter is the sole error-logging owner. HTTP exposes only Unavailable.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consume the sensitive cause at its sole logging and sanitization boundary"
)]
pub(crate) fn unavailable(error: sqlx::Error, operation: &'static str) -> domain::Error {
    let category = Failure::classify(&error);
    tracing::error!(
        operation,
        storage_category = category.label(),
        "storage operation failed"
    );
    domain::Error::Unavailable
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sensitive_io_causes_become_closed_categories() {
        let error = sqlx::Error::Io(std::io::Error::other("postgres://sensitive:password@host"));
        assert_eq!(Failure::classify(&error), Failure::Connection);
        assert_eq!(
            Failure::classify(&sqlx::Error::PoolTimedOut),
            Failure::Timeout
        );
    }
}
