use super::Database;
use application::Actor;
use application::authentication::{CurrentAccount, IdentityRepository};
use async_trait::async_trait;
use domain::{Error, SessionId};
use sha2::{Digest, Sha256};
use uuid::Uuid;
// Callers retain their own executor and commit boundary.
const ADMIT_ATTEMPT: &str = r"
    INSERT INTO auth_limits(key_hash, window_start, attempts)
    VALUES($1, clock_timestamp(), 1)
    ON CONFLICT(key_hash) DO UPDATE SET
        attempts = CASE
            WHEN auth_limits.window_start < clock_timestamp() - interval '15 minutes' THEN 1
            ELSE auth_limits.attempts + 1
        END,
        window_start = CASE
            WHEN auth_limits.window_start < clock_timestamp() - interval '15 minutes' THEN clock_timestamp()
            ELSE auth_limits.window_start
        END
    RETURNING attempts
";
#[must_use]
pub fn token_hash(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}
fn storage(error: sqlx::Error) -> Error {
    super::errors::unavailable(error, "auth")
}

#[async_trait]
impl IdentityRepository for Database {
    async fn admit_source(&self, source: &str) -> Result<(), Error> {
        let key = token_hash(&format!("auth-source:{source}"));
        let attempts: i32 = sqlx::query_scalar(ADMIT_ATTEMPT)
            .bind(key)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)?;
        if attempts > 120 {
            Err(Error::RateLimited)
        } else {
            Ok(())
        }
    }
    async fn issue_login(&self, email: &str, token: &str) -> Result<(), Error> {
        let mut tx = riverqueue::database::begin_postgres(&self.pool)
            .await
            .map_err(storage)?;
        let attempts: i32 = sqlx::query_scalar(ADMIT_ATTEMPT)
            .bind(token_hash(email))
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        if attempts > 5 {
            tx.commit().await.map_err(storage)?;
            return Err(Error::RateLimited);
        }
        sqlx::query("INSERT INTO magic_links(token_hash,email) VALUES($1,$2)")
            .bind(token_hash(token))
            .bind(email)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)
    }
    async fn consume_login(&self, token: &str, session: &str) -> Result<Actor, Error> {
        if token.len() != 64 {
            return Err(Error::Unauthenticated);
        }
        // The token, not caller-supplied email, determines the mailbox bucket.
        // Admission commits outside consumption so failed/replayed attempts count.
        let mailbox: Option<String> =
            sqlx::query_scalar("SELECT email FROM magic_links WHERE token_hash=$1")
                .bind(token_hash(token))
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?;
        if let Some(email) = mailbox {
            let attempts: i32 = sqlx::query_scalar(ADMIT_ATTEMPT)
                .bind(token_hash(&format!("confirm-email:{email}")))
                .fetch_one(&self.pool)
                .await
                .map_err(storage)?;
            if attempts > 5 {
                return Err(Error::RateLimited);
            }
        }
        let mut tx = riverqueue::database::begin_postgres(&self.pool)
            .await
            .map_err(storage)?;
        let email: Option<String> = sqlx::query_scalar(
            r"
            UPDATE magic_links
            SET consumed_at=clock_timestamp()
            WHERE token_hash=$1
            AND consumed_at IS NULL
            AND expires_at>clock_timestamp()
            RETURNING email
            ",
        )
        .bind(token_hash(token))
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?;
        let email = email.ok_or(Error::Unauthenticated)?;
        let (user_id, disabled): (Uuid, bool) = sqlx::query_as(
            r"
            INSERT INTO users(id,email)
            VALUES($1,$2)
            ON CONFLICT(email)
            DO UPDATE SET email=EXCLUDED.email
            RETURNING id,disabled
            ",
        )
        .bind(Uuid::now_v7())
        .bind(email)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        if disabled {
            tx.commit().await.map_err(storage)?;
            return Err(Error::Unauthenticated);
        }
        // The helper borrows this transaction; link consumption, user upsert
        // and session creation are published by the single commit below.
        let session_id = super::sessions::insert_session(&mut tx, user_id, session).await?;
        tx.commit().await.map_err(storage)?;
        Ok(Actor {
            correlation_id: Uuid::now_v7(),
            user_id: user_id.into(),
            session_id: session_id.into(),
        })
    }
    async fn authenticate(&self, token: &str) -> Result<Actor, Error> {
        self.session_authenticate(token).await
    }
    async fn current_account(&self, token: &str) -> Result<CurrentAccount, Error> {
        self.session_current_account(token).await
    }
    async fn revoke(&self, token: &str) -> Result<(), Error> {
        self.session_revoke(token).await
    }
    async fn list_sessions(
        &self,
        actor: &Actor,
        after: Option<SessionId>,
    ) -> Result<Vec<SessionId>, Error> {
        self.session_list_sessions(actor, after).await
    }
    async fn rotate(&self, actor: &Actor, old: &str, new: &str) -> Result<(), Error> {
        self.session_rotate(actor, old, new).await
    }
    async fn revoke_session(&self, actor: &Actor, session: SessionId) -> Result<(), Error> {
        self.session_revoke_session(actor, session).await
    }
    async fn revoke_all(&self, token: &str) -> Result<(), Error> {
        self.session_revoke_all(token).await
    }
}

impl Database {
    /// Global credential garbage collection only; each deletion is retry-safe.
    pub(crate) async fn cleanup_identity_records(&self) -> Result<(), Error> {
        sqlx::query("DELETE FROM magic_links WHERE expires_at<clock_timestamp()-interval '1 day'")
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        sqlx::query(
            "DELETE FROM auth_limits WHERE window_start<clock_timestamp()-interval '1 day'",
        )
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(())
    }
}
