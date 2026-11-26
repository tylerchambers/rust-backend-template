#![allow(
    clippy::unwrap_used,
    reason = "Unexpected test results must fail immediately"
)]

use application::{
    Actor,
    authority::{AuthorityMutation, TenantAuthority},
    memberships::{MembershipRepository, MembershipTransaction, MembershipUnitOfWork, Memberships},
    transaction::TenantTransaction,
};
use async_trait::async_trait;
use domain::{Error, OrganizationId, UserId};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Clone, Default, Debug, PartialEq)]
struct State {
    members: BTreeMap<UserId, bool>,
    retired: Vec<UserId>,
    events: Vec<(String, Uuid)>,
}

#[derive(Clone, Default)]
struct Memory(Arc<Mutex<State>>);

struct Work {
    memory: Memory,
    state: State,
    organization_locked: bool,
    users: Vec<UserId>,
}

#[async_trait]
impl MembershipUnitOfWork for Memory {
    async fn begin_memberships(
        &self,
        _: OrganizationId,
    ) -> Result<Box<dyn MembershipTransaction>, Error> {
        Ok(Box::new(Work {
            memory: self.clone(),
            state: self.0.lock().unwrap().clone(),
            organization_locked: false,
            users: vec![],
        }))
    }
}

#[async_trait]
impl AuthorityMutation for Work {
    async fn lock_organization(&mut self) -> Result<(), Error> {
        self.organization_locked = true;
        Ok(())
    }
    async fn lock_users(&mut self, users: &[UserId]) -> Result<(), Error> {
        assert!(self.organization_locked);
        self.users.extend_from_slice(users);
        Ok(())
    }
}

#[async_trait]
impl TenantAuthority for Work {
    async fn is_admin(&mut self, actor: &Actor, _: OrganizationId) -> Result<bool, Error> {
        assert!(self.users.contains(&actor.user_id));
        self.state
            .members
            .get(&actor.user_id)
            .copied()
            .ok_or(Error::Forbidden)
    }
}

#[async_trait]
impl MembershipRepository for Work {
    async fn member_is_admin(&mut self, user: UserId) -> Result<bool, Error> {
        assert!(self.users.contains(&user));
        self.state
            .members
            .get(&user)
            .copied()
            .ok_or(Error::NotFound)
    }
    async fn admin_count(&mut self) -> Result<i64, Error> {
        assert!(self.organization_locked);
        Ok(self
            .state
            .members
            .values()
            .map(|admin| i64::from(*admin))
            .sum())
    }
    async fn set_admin(&mut self, user: UserId, is_admin: bool) -> Result<(), Error> {
        *self.state.members.get_mut(&user).ok_or(Error::NotFound)? = is_admin;
        Ok(())
    }
    async fn retire_sessions(&mut self, user: UserId) -> Result<(), Error> {
        self.state.retired.push(user);
        Ok(())
    }
    async fn remove_member(&mut self, user: UserId) -> Result<(), Error> {
        self.state.members.remove(&user).ok_or(Error::NotFound)?;
        Ok(())
    }
}

#[async_trait]
impl TenantTransaction for Work {
    async fn audit(&mut self, _: &Actor, action: &str, subject: Uuid) -> Result<(), Error> {
        self.state.events.push((action.into(), subject));
        Ok(())
    }
    async fn commit(self: Box<Self>) -> Result<(), Error> {
        *self.memory.0.lock().unwrap() = self.state;
        Ok(())
    }
}

fn actor() -> Actor {
    Actor {
        user_id: Uuid::now_v7().into(),
        session_id: Uuid::now_v7().into(),
        correlation_id: Uuid::now_v7(),
    }
}

#[tokio::test]
async fn last_admin_is_protected_and_members_cannot_manage_membership() {
    let memory = Memory::default();
    let service = Memberships::new(Arc::new(memory.clone()));
    let org = Uuid::now_v7().into();
    let admin = actor();
    let member = actor();
    let initial = State {
        members: BTreeMap::from([(admin.user_id, true), (member.user_id, false)]),
        ..State::default()
    };
    *memory.0.lock().unwrap() = initial.clone();

    assert!(matches!(
        service.remove(&admin, org, admin.user_id).await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        service.change_role(&admin, org, admin.user_id, false).await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        service.remove(&member, org, admin.user_id).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service
            .change_role(&member, org, member.user_id, true)
            .await,
        Err(Error::Forbidden)
    ));
    assert_eq!(*memory.0.lock().unwrap(), initial);
}

#[tokio::test]
async fn real_admin_changes_retire_sessions_but_no_ops_and_removal_do_not() {
    let memory = Memory::default();
    let service = Memberships::new(Arc::new(memory.clone()));
    let org = Uuid::now_v7().into();
    let admin = actor();
    let member = actor();
    memory.0.lock().unwrap().members =
        BTreeMap::from([(admin.user_id, true), (member.user_id, false)]);

    service
        .change_role(&admin, org, admin.user_id, true)
        .await
        .unwrap();
    service
        .change_role(&admin, org, member.user_id, true)
        .await
        .unwrap();
    service
        .change_role(&admin, org, member.user_id, false)
        .await
        .unwrap();
    service
        .change_role(&admin, org, member.user_id, true)
        .await
        .unwrap();
    service.remove(&admin, org, admin.user_id).await.unwrap();
    let state = memory.0.lock().unwrap();
    assert_eq!(state.members, BTreeMap::from([(member.user_id, true)]));
    assert_eq!(state.retired, [member.user_id; 3]);
    assert_eq!(
        state.events,
        [
            ("membership.role_changed".into(), admin.user_id.into_uuid()),
            ("membership.role_changed".into(), member.user_id.into_uuid()),
            ("membership.role_changed".into(), member.user_id.into_uuid()),
            ("membership.role_changed".into(), member.user_id.into_uuid()),
            ("membership.removed".into(), admin.user_id.into_uuid()),
        ]
    );
}
