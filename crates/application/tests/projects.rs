#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::{
    Actor, IdGenerator, Projects, authority::TenantAuthority, projects::*,
    transaction::TenantTransaction,
};
use async_trait::async_trait;
use domain::{Error, InvitationId, OrganizationId, Project, ProjectId, UserId};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;
#[path = "support/project_contract.rs"]
mod contract;

#[derive(Clone, Default)]
struct State {
    projects: BTreeMap<Uuid, Project>,
    jobs: Vec<ProjectId>,
    audits: Vec<String>,
}
struct Memory {
    state: Arc<Mutex<State>>,
    member: UserId,
    org: OrganizationId,
    fail_audit: bool,
}
struct Work {
    state: State,
    published: Arc<Mutex<State>>,
    member: UserId,
    org: OrganizationId,
    requested: OrganizationId,
    fail_audit: bool,
}
struct Ids;
impl IdGenerator for Ids {
    fn organization(&self) -> OrganizationId {
        Uuid::now_v7().into()
    }
    fn project(&self) -> ProjectId {
        Uuid::now_v7().into()
    }
    fn invitation(&self) -> InvitationId {
        Uuid::now_v7().into()
    }
}
#[async_trait]
impl ProjectUnitOfWork for Memory {
    async fn begin_projects(
        &self,
        requested: OrganizationId,
    ) -> Result<Box<dyn ProjectTransaction>, Error> {
        Ok(Box::new(Work {
            state: self.state.lock().unwrap().clone(),
            published: self.state.clone(),
            member: self.member,
            org: self.org,
            requested,
            fail_audit: self.fail_audit,
        }))
    }
}
#[async_trait]
impl TenantAuthority for Work {
    async fn is_admin(&mut self, actor: &Actor, org: OrganizationId) -> Result<bool, Error> {
        if actor.user_id == self.member && org == self.org && org == self.requested {
            Ok(false)
        } else {
            Err(Error::Forbidden)
        }
    }
}
#[async_trait]
impl ProjectRepository for Work {
    async fn enqueue_project_change(&mut self, id: ProjectId, _: Uuid) -> Result<(), Error> {
        self.state.jobs.push(id);
        Ok(())
    }

    async fn insert_project(&mut self, p: &Project) -> Result<(), Error> {
        self.state.projects.insert(p.id.into_uuid(), p.clone());
        Ok(())
    }
    async fn load_project(&mut self, id: ProjectId) -> Result<Project, Error> {
        self.state
            .projects
            .get(&id.into_uuid())
            .filter(|p| p.organization_id == self.requested)
            .cloned()
            .ok_or(Error::NotFound)
    }
    async fn save_project(&mut self, p: &Project, expected: i64) -> Result<(), Error> {
        if self.load_project(p.id).await?.version != expected {
            return Err(Error::Conflict);
        }
        self.state.projects.insert(p.id.into_uuid(), p.clone());
        Ok(())
    }
    async fn list_projects(&mut self, after: Option<ProjectId>) -> Result<Vec<Project>, Error> {
        Ok(self
            .state
            .projects
            .values()
            .filter(|p| {
                p.organization_id == self.requested
                    && after.is_none_or(|a| p.id.into_uuid() > a.into_uuid())
            })
            .take(50)
            .cloned()
            .collect())
    }
}
#[async_trait]
impl TenantTransaction for Work {
    async fn audit(&mut self, _: &Actor, action: &str, _: Uuid) -> Result<(), Error> {
        if self.fail_audit {
            return Err(Error::Unavailable);
        }
        self.state.audits.push(action.into());
        Ok(())
    }
    async fn commit(self: Box<Self>) -> Result<(), Error> {
        *self.published.lock().unwrap() = self.state;
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
async fn memory_obeys_shared_project_contract() {
    let member = actor();
    let org = Uuid::now_v7().into();
    let memory = Arc::new(Memory {
        state: Arc::default(),
        member: member.user_id,
        org,
        fail_audit: false,
    });
    contract::project_contract(
        &Projects::new(memory.clone(), Arc::new(Ids)),
        &member,
        &actor(),
        org,
    )
    .await;
    let state = memory.state.lock().unwrap();
    assert_eq!(state.jobs.len(), 2);
    assert_eq!(state.audits.len(), 2);
}
#[tokio::test]
async fn audit_failure_publishes_neither_project_nor_job() {
    let member = actor();
    let org = Uuid::now_v7().into();
    let memory = Arc::new(Memory {
        state: Arc::default(),
        member: member.user_id,
        org,
        fail_audit: true,
    });
    let service = Projects::new(memory.clone(), Arc::new(Ids));
    assert!(matches!(
        service.create(&member, org, "Rollback".into()).await,
        Err(Error::Unavailable)
    ));
    let state = memory.state.lock().unwrap();
    assert!(state.projects.is_empty());
    assert_eq!(state.jobs, [] as [domain::ProjectId; 0]);
    assert_eq!(state.audits, [] as [std::string::String; 0]);
}
