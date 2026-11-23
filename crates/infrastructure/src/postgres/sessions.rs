use super::{Database, authentication::token_hash};
use application::Actor;
use application::authentication::CurrentAccount;
use domain::{Error, SessionId};
use uuid::Uuid;
fn storage(error: sqlx::Error) -> Error {
    super::errors::unavailable(error, "auth")
}
macro_rules! authenticated_session_sql {
    ($projection:literal) => {
        concat!(
            r"
            UPDATE sessions s
            SET last_seen_at=clock_timestamp()
            FROM users u
            WHERE s.token_hash=$1
            AND u.id=s.user_id
            AND NOT u.disabled
            AND s.revoked_at IS NULL
            AND s.expires_at>clock_timestamp()
            AND s.last_seen_at>clock_timestamp()-interval '30 minutes'",
            " RETURNING ",
            $projection
        )
    };
}
impl Database {
    pub(super) async fn session_authenticate(&self, token: &str) -> Result<Actor, Error> {
        if token.len() != 64 {
            return Err(Error::Unauthenticated);
        }
        let row: Option<(Uuid, Uuid)> =
            sqlx::query_as(authenticated_session_sql!("s.user_id,s.id"))
                .bind(token_hash(token))
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?;
        row.map(|(user_id, session_id)| Actor {
            correlation_id: Uuid::now_v7(),
            user_id: user_id.into(),
            session_id: session_id.into(),
        })
        .ok_or(Error::Unauthenticated)
    }
    pub(super) async fn session_current_account(
        &self,
        token: &str,
    ) -> Result<CurrentAccount, Error> {
        if token.len() != 64 {
            return Err(Error::Unauthenticated);
        }
        let row: Option<(Uuid, String)> =
            sqlx::query_as(authenticated_session_sql!("s.user_id,u.email"))
                .bind(token_hash(token))
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?;
        row.map(|(id, email)| CurrentAccount {
            user_id: id.into(),
            email,
        })
        .ok_or(Error::Unauthenticated)
    }
    pub(super) async fn session_revoke(&self, token: &str) -> Result<(), Error> {
        // Resolve immutable identity before waiting on a rotating session row. A
        // single UPDATE can recheck the new hash against a pre-rotation snapshot
        // that cannot yet see the revocation handle and silently miss the row.
        let id: Option<Uuid> = sqlx::query_scalar(
            r"
            SELECT id
            FROM sessions
            WHERE token_hash=$1
            UNION SELECT session_id
            FROM session_revocation_handles
            WHERE token_hash=$1
            ",
        )
        .bind(token_hash(token))
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        if let Some(id) = id {
            sqlx::query(
                "UPDATE sessions SET revoked_at=clock_timestamp() WHERE id=$1 AND revoked_at IS NULL",
            )
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        }
        Ok(())
    }
    pub(super) async fn session_list_sessions(
        &self,
        actor: &Actor,
        after: Option<SessionId>,
    ) -> Result<Vec<SessionId>, Error> {
        sqlx::query_scalar::<_, Uuid>(
            r"
            SELECT id
            FROM sessions
            WHERE user_id=$1
            AND revoked_at IS NULL
            AND expires_at>clock_timestamp()
            AND last_seen_at>clock_timestamp()-interval '30 minutes'
            AND ($2::uuid IS NULL OR id>$2)
            ORDER BY id
            LIMIT 50
            ",
        )
        .bind(actor.user_id.into_uuid())
        .bind(after.map(SessionId::into_uuid))
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(SessionId::from).collect())
        .map_err(storage)
    }
    pub(super) async fn session_rotate(
        &self,
        actor: &Actor,
        old: &str,
        new: &str,
    ) -> Result<(), Error> {
        let mut tx = riverqueue::database::begin_postgres(&self.pool)
            .await
            .map_err(storage)?;
        let n = sqlx::query(
            r"
            UPDATE sessions
            SET token_hash=$1
            WHERE id=$2
            AND user_id=$3
            AND token_hash=$4
            AND revoked_at IS NULL
            AND expires_at>clock_timestamp()
            ",
        )
        .bind(token_hash(new))
        .bind(actor.session_id.into_uuid())
        .bind(actor.user_id.into_uuid())
        .bind(token_hash(old))
        .execute(&mut *tx)
        .await
        .map_err(storage)?
        .rows_affected();
        if n != 1 {
            return Err(Error::Unauthenticated);
        }
        sqlx::query("INSERT INTO session_revocation_handles(token_hash,session_id) VALUES($1,$2)")
            .bind(token_hash(old))
            .bind(actor.session_id.into_uuid())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)
    }
    pub(super) async fn session_revoke_session(
        &self,
        actor: &Actor,
        session: SessionId,
    ) -> Result<(), Error> {
        let changed = sqlx::query(
            r"
            UPDATE sessions
            SET revoked_at=COALESCE(revoked_at,clock_timestamp())
            WHERE id=$1
            AND user_id=$2
            AND EXISTS(SELECT 1
            FROM sessions caller
            JOIN users u ON u.id=caller.user_id
            WHERE caller.id=$3
            AND caller.user_id=$2
            AND caller.revoked_at IS NULL
            AND caller.expires_at>clock_timestamp()
            AND caller.last_seen_at>clock_timestamp()-interval '30 minutes'
            AND NOT u.disabled)
            ",
        )
        .bind(session.into_uuid())
        .bind(actor.user_id.into_uuid())
        .bind(actor.session_id.into_uuid())
        .execute(&self.pool)
        .await
        .map_err(storage)?
        .rows_affected();
        if changed == 1 {
            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }
    pub(super) async fn session_revoke_all(&self, token: &str) -> Result<(), Error> {
        if token.len() != 64 {
            return Err(Error::Unauthenticated);
        }
        let hash = token_hash(token);
        let mut tx = riverqueue::database::begin_postgres(&self.pool)
            .await
            .map_err(storage)?;
        // Login holds this identity lock from its upsert through session insertion.
        // Take it before any session locks; the next statement sees preceding logins.
        let user_id: Uuid = sqlx::query_scalar(
            r"
            SELECT u.id FROM users u JOIN sessions s ON s.user_id=u.id
            WHERE s.token_hash=$1 AND s.revoked_at IS NULL AND NOT u.disabled
            FOR UPDATE OF u
            ",
        )
        .bind(&hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?
        .ok_or(Error::Unauthenticated)?;
        // Acquire every row lock the mutation needs before its database-time authority check.
        sqlx::query(
            r"
            SELECT count(*) FROM (
                SELECT id FROM sessions WHERE user_id=$1 AND revoked_at IS NULL FOR NO KEY UPDATE
            ) locked
            ",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        let retired = sqlx::query(
            r"
            UPDATE sessions SET revoked_at=clock_timestamp()
            WHERE user_id=$1 AND revoked_at IS NULL AND EXISTS (
                SELECT 1 FROM sessions caller
                WHERE caller.user_id=$1 AND caller.token_hash=$2 AND caller.revoked_at IS NULL
                AND caller.expires_at>clock_timestamp()
                AND caller.last_seen_at>clock_timestamp()-interval '30 minutes'
            )
            ",
        )
        .bind(user_id)
        .bind(hash)
        .execute(&mut *tx)
        .await
        .map_err(storage)?
        .rows_affected();
        if retired == 0 {
            return Err(Error::Unauthenticated);
        }
        tx.commit().await.map_err(storage)
    }
}

/// Session insertion participates in login's caller-owned transaction only.
pub(super) async fn insert_session(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    token: &str,
) -> Result<Uuid, Error> {
    let session_id = Uuid::now_v7();
    sqlx::query("INSERT INTO sessions(id,user_id,token_hash) VALUES($1,$2,$3)")
        .bind(session_id)
        .bind(user_id)
        .bind(token_hash(token))
        .execute(&mut **tx)
        .await
        .map_err(storage)?;
    Ok(session_id)
}
