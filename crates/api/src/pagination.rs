use serde::Deserialize;
use uuid::Uuid;

/// Exclusive UUID cursor shared by bounded resource discovery endpoints.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UuidCursor {
    pub(crate) after: Option<Uuid>,
}
