use crate::metrics::HttpMetrics;
use application::{
    Authentication, Organizations, Projects, Secrets, administration::Administration,
    invitations::Invitations, memberships::Memberships, reauthentication::Reauthentication,
};
use prometheus_client::registry::Registry;
use std::sync::Arc;

#[derive(Clone)]
pub struct State {
    pub(crate) metrics: Arc<HttpMetrics>,
    pub(crate) auth: Arc<Authentication>,
    pub(crate) organizations: Arc<Organizations>,
    pub(crate) projects: Arc<Projects>,
    pub(crate) secrets: Arc<dyn Secrets>,
    pub(crate) origin: String,
    pub(crate) invitations: Arc<Invitations>,
    pub(crate) memberships: Arc<Memberships>,
    pub(crate) reauthentication: Arc<Reauthentication>,
    pub(crate) administration: Arc<Administration>,
}
impl State {
    /// Register this application's instruments in the composition root's registry.
    pub fn register_metrics(&self, registry: &mut Registry) {
        self.metrics.register(registry);
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "Explicit injected resource services keep composition dependencies visible"
    )]
    pub fn new(
        auth: Arc<Authentication>,
        organizations: Arc<Organizations>,
        projects: Arc<Projects>,
        secrets: Arc<dyn Secrets>,
        origin: String,
        memberships: Arc<Memberships>,
        invitations: Arc<Invitations>,
        reauthentication: Arc<Reauthentication>,
        administration: Arc<Administration>,
    ) -> Self {
        Self {
            metrics: Arc::new(HttpMetrics::default()),
            auth,
            organizations,
            projects,
            secrets,
            origin,
            memberships,
            invitations,
            reauthentication,
            administration,
        }
    }
}
