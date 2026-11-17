use riverqueue::JobArgs;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Debug, Serialize, Deserialize, JobArgs)]
#[river(kind = "project_changed_v1")]
pub struct ProjectChanged {
    #[serde(default)]
    pub correlation_id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize, JobArgs)]
#[river(kind = "identity_cleanup_v1")]
pub struct Cleanup {}

#[derive(Clone, Debug, Serialize, Deserialize, JobArgs)]
#[river(kind = "invitation_email_v1")]
pub struct InvitationEmail {
    // Old enqueued payloads remain decodable during a rolling upgrade.
    #[serde(default)]
    pub correlation_id: uuid::Uuid,
    pub organization_id: uuid::Uuid,
    pub invitation_id: uuid::Uuid,
}
