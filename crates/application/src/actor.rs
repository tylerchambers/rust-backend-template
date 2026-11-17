use domain::{SessionId, UserId};
use uuid::Uuid;

/// Authenticated invocation context, not a snapshot of current tenant authority.
#[derive(Clone)]
pub struct Actor {
    /// Non-secret server-generated identifier for this invocation.
    pub correlation_id: Uuid,
    pub user_id: UserId,
    pub session_id: SessionId,
}
