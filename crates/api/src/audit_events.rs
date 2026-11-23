use crate::{
    authentication::actor,
    error::{ApiError, ErrorBody},
    pagination::UuidCursor,
    state::State,
};
use axum::{
    Json,
    extract::{Path, Query, State as Extract},
    http::HeaderMap,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub struct AuditEventResponse {
    id: Uuid,
    actor_id: Uuid,
    action: String,
    subject: Uuid,
    occurred_at: DateTime<Utc>,
}

#[utoipa::path(
    get,
    path="/v1/organizations/{organization}/audit-events",
    params(
        ("organization"=Uuid,Path),
        ("after"=Option<Uuid>,Query,description="Exclusive event ID cursor. Up to 50 rows in ascending ID order, not commit order; continue with the last ID until empty."),
    ),
    security(("session"=[])),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=200,body=Vec<AuditEventResponse>),
    )
)]
pub(crate) async fn list_audit_events(
    Extract(state): Extract<State>,
    Path(org): Path<Uuid>,
    Query(page): Query<UuidCursor>,
    headers: HeaderMap,
) -> Result<Json<Vec<AuditEventResponse>>, ApiError> {
    let actor = actor(&state, &headers).await?;
    Ok(Json(
        state
            .administration
            .audit_events(&actor, org.into(), page.after)
            .await?
            .into_iter()
            .map(|row| AuditEventResponse {
                id: row.id,
                actor_id: row.actor_id.into_uuid(),
                action: row.action,
                subject: row.subject,
                occurred_at: row.occurred_at,
            })
            .collect(),
    ))
}
