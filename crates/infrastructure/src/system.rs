use application::Secrets;
use rand::RngExt;
use uuid::Uuid;

pub struct SystemIds;
impl application::IdGenerator for SystemIds {
    fn organization(&self) -> domain::OrganizationId {
        Uuid::now_v7().into()
    }
    fn project(&self) -> domain::ProjectId {
        Uuid::now_v7().into()
    }
    fn invitation(&self) -> domain::InvitationId {
        Uuid::now_v7().into()
    }
}
pub struct SystemClock;
impl application::Clock for SystemClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }
}

pub struct SystemSecrets;
impl Secrets for SystemSecrets {
    fn token(&self) -> String {
        hex::encode(rand::rng().random::<[u8; 32]>())
    }
}
