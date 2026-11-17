#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("invalid_input")]
    Invalid,
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("forbidden")]
    Forbidden,
    #[error("not_found")]
    NotFound,
    #[error("conflict")]
    Conflict,
    #[error("rate_limited")]
    RateLimited,
    #[error("unavailable")]
    Unavailable,
}
