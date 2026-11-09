//! Explicit identity -> session ordering shared by tenant authority readers.
use application::Actor;
use domain::Error;
use sqlx::{Postgres, Transaction};

fn storage(error: sqlx::Error) -> Error {
    crate::storage::unavailable(error, "identity")
}

pub(crate) async fn lock_actor(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
) -> Result<(), Error> {
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR SHARE")
        .bind(actor.user_id.into_uuid())
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage)?;
    sqlx::query("SELECT id FROM sessions WHERE id=$1 AND user_id=$2 FOR SHARE")
        .bind(actor.session_id.into_uuid())
        .bind(actor.user_id.into_uuid())
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage)?;
    Ok(())
}

/// Recheck time-sensitive authority in a fresh statement after all lock waits.
/// Caller must already hold the identity and session locks until publication.
pub(crate) async fn live_email(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
) -> Result<Option<String>, Error> {
    sqlx::query_scalar(
        r"
        SELECT u.email
        FROM users u JOIN sessions s ON s.user_id=u.id
        WHERE u.id=$1 AND s.id=$2
        AND NOT u.disabled AND s.revoked_at IS NULL
        AND s.expires_at>clock_timestamp()
        AND s.last_seen_at>clock_timestamp()-interval '30 minutes'
        ",
    )
    .bind(actor.user_id.into_uuid())
    .bind(actor.session_id.into_uuid())
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage)
}
