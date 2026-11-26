#![allow(
    clippy::unwrap_used,
    reason = "Test setup and assertions fail immediately on unexpected errors"
)]

use application::{
    Actor, IdGenerator, Organizations,
    authority::TenantAuthority,
    organizations::{
        MemberSummary, OrganizationRepository, OrganizationSummary, OrganizationTransaction,
        OrganizationUnitOfWork,
    },
    transaction::TenantTransaction,
};
use async_trait::async_trait;
use domain::{Error, InvitationId, OrganizationId, ProjectId, UserId};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

struct Discovery {
    live: bool,
    committed: Arc<Mutex<bool>>,
}

struct DiscoveryWork {
    live: bool,
    validated: Option<UserId>,
    committed: Arc<Mutex<bool>>,
}

#[async_trait]
impl OrganizationUnitOfWork for Discovery {
    async fn begin_discovery(&self) -> Result<Box<dyn OrganizationTransaction>, Error> {
        Ok(Box::new(DiscoveryWork {
            live: self.live,
            validated: None,
            committed: self.committed.clone(),
        }))
    }

    async fn begin_organization(
        &self,
        _: OrganizationId,
    ) -> Result<Box<dyn OrganizationTransaction>, Error> {
        // Discovery must never select a tenant, even a nil sentinel.
        Err(Error::Forbidden)
    }
}

#[async_trait]
impl TenantAuthority for DiscoveryWork {
    async fn is_admin(&mut self, _: &Actor, _: OrganizationId) -> Result<bool, Error> {
        Err(Error::Forbidden)
    }
}

#[async_trait]
impl OrganizationRepository for DiscoveryWork {
    async fn validate_actor(&mut self, actor: &Actor) -> Result<(), Error> {
        if !self.live {
            return Err(Error::Unauthenticated);
        }
        self.validated = Some(actor.user_id);
        Ok(())
    }

    async fn list_organizations(
        &mut self,
        actor: &Actor,
        after: Option<OrganizationId>,
    ) -> Result<Vec<OrganizationSummary>, Error> {
        if self.validated != Some(actor.user_id) {
            return Err(Error::Unauthenticated);
        }
        let id = Uuid::from_u128(2).into();
        Ok(if after.is_none_or(|cursor| cursor < id) {
            vec![OrganizationSummary {
                id,
                name: "Discovered".into(),
                is_admin: false,
            }]
        } else {
            vec![]
        })
    }

    async fn list_members(&mut self, _: Option<UserId>) -> Result<Vec<MemberSummary>, Error> {
        Err(Error::Forbidden)
    }

    async fn insert_organization(&mut self, _: OrganizationId, _: &str) -> Result<(), Error> {
        Err(Error::Forbidden)
    }

    async fn insert_admin(&mut self, _: OrganizationId, _: UserId) -> Result<(), Error> {
        Err(Error::Forbidden)
    }
}

#[async_trait]
impl TenantTransaction for DiscoveryWork {
    async fn audit(&mut self, _: &Actor, _: &str, _: Uuid) -> Result<(), Error> {
        Err(Error::Forbidden)
    }

    async fn commit(self: Box<Self>) -> Result<(), Error> {
        *self.committed.lock().unwrap() = true;
        Ok(())
    }
}

struct UnusedIds;
impl IdGenerator for UnusedIds {
    fn organization(&self) -> OrganizationId {
        Uuid::nil().into()
    }
    fn project(&self) -> ProjectId {
        Uuid::nil().into()
    }
    fn invitation(&self) -> InvitationId {
        Uuid::nil().into()
    }
}

fn actor() -> Actor {
    Actor {
        correlation_id: Uuid::now_v7(),
        user_id: Uuid::now_v7().into(),
        session_id: Uuid::now_v7().into(),
    }
}

#[tokio::test]
async fn discovery_uses_account_scope_and_preserves_exclusive_cursor() {
    let committed = Arc::new(Mutex::new(false));
    let service = Organizations::new(
        Arc::new(Discovery {
            live: true,
            committed: committed.clone(),
        }),
        Arc::new(UnusedIds),
    );
    let actor = actor();
    let rows = service.list(&actor, None).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, Uuid::from_u128(2).into());
    assert_eq!(rows[0].name, "Discovered");
    assert!(!rows[0].is_admin);
    assert!(*committed.lock().unwrap());
    assert!(
        service
            .list(&actor, Some(rows[0].id))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn discovery_rejects_stale_actor_without_committing() {
    let committed = Arc::new(Mutex::new(false));
    let service = Organizations::new(
        Arc::new(Discovery {
            live: false,
            committed: committed.clone(),
        }),
        Arc::new(UnusedIds),
    );
    assert!(matches!(
        service.list(&actor(), None).await,
        Err(Error::Unauthenticated)
    ));
    assert!(!*committed.lock().unwrap());
}
