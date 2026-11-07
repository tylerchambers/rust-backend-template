use crate::Database;
use application::{Actor, IdentityRepository, Secrets};
use async_trait::async_trait;
use domain::Error;
use domain::SessionId;
use rand::RngExt;
use sha2::{Digest, Sha256};
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
#[must_use]
pub fn token_hash(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}
fn storage(error: sqlx::Error) -> Error {
    crate::storage::unavailable(error, "auth")
}

#[async_trait]
impl IdentityRepository for Database {
    async fn admit_source(&self, source: &str) -> Result<(), Error> {
        let key = token_hash(&format!("auth-source:{source}"));
        let attempts: i32 = sqlx::query_scalar("INSERT INTO auth_limits(key_hash,window_start,attempts) VALUES($1,clock_timestamp(),1) ON CONFLICT(key_hash) DO UPDATE SET attempts=CASE WHEN auth_limits.window_start < clock_timestamp()-interval '15 minutes' THEN 1 ELSE auth_limits.attempts+1 END, window_start=CASE WHEN auth_limits.window_start < clock_timestamp()-interval '15 minutes' THEN clock_timestamp() ELSE auth_limits.window_start END RETURNING attempts").bind(key).fetch_one(&self.pool).await.map_err(storage)?;
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
        let attempts:i32=sqlx::query_scalar("INSERT INTO auth_limits(key_hash,window_start,attempts) VALUES($1,clock_timestamp(),1) ON CONFLICT(key_hash) DO UPDATE SET attempts=CASE WHEN auth_limits.window_start < clock_timestamp()-interval '15 minutes' THEN 1 ELSE auth_limits.attempts+1 END, window_start=CASE WHEN auth_limits.window_start < clock_timestamp()-interval '15 minutes' THEN clock_timestamp() ELSE auth_limits.window_start END RETURNING attempts")
        .bind(token_hash(email)).fetch_one(&mut *tx).await.map_err(storage)?;
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
            let attempts: i32 = sqlx::query_scalar("INSERT INTO auth_limits(key_hash,window_start,attempts) VALUES($1,clock_timestamp(),1) ON CONFLICT(key_hash) DO UPDATE SET attempts=CASE WHEN auth_limits.window_start < clock_timestamp()-interval '15 minutes' THEN 1 ELSE auth_limits.attempts+1 END, window_start=CASE WHEN auth_limits.window_start < clock_timestamp()-interval '15 minutes' THEN clock_timestamp() ELSE auth_limits.window_start END RETURNING attempts")
                .bind(token_hash(&format!("confirm-email:{email}"))).fetch_one(&self.pool).await.map_err(storage)?;
            if attempts > 5 {
                return Err(Error::RateLimited);
            }
        }
        let mut tx = riverqueue::database::begin_postgres(&self.pool)
            .await
            .map_err(storage)?;
        let email:Option<String>=sqlx::query_scalar("UPDATE magic_links SET consumed_at=clock_timestamp() WHERE token_hash=$1 AND consumed_at IS NULL AND expires_at>clock_timestamp() RETURNING email").bind(token_hash(token)).fetch_optional(&mut *tx).await.map_err(storage)?;
        let email = email.ok_or(Error::Unauthenticated)?;
        let (user_id,disabled):(Uuid,bool)=sqlx::query_as("INSERT INTO users(id,email) VALUES($1,$2) ON CONFLICT(email) DO UPDATE SET email=EXCLUDED.email RETURNING id,disabled").bind(Uuid::now_v7()).bind(email).fetch_one(&mut *tx).await.map_err(storage)?;
        if disabled {
            tx.commit().await.map_err(storage)?;
            return Err(Error::Unauthenticated);
        }
        let session_id = Uuid::now_v7();
        sqlx::query("INSERT INTO sessions(id,user_id,token_hash) VALUES($1,$2,$3)")
            .bind(session_id)
            .bind(user_id)
            .bind(token_hash(session))
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(Actor {
            correlation_id: Uuid::now_v7(),
            user_id: user_id.into(),
            session_id: session_id.into(),
        })
    }
    async fn authenticate(&self, token: &str) -> Result<Actor, Error> {
        if token.len() != 64 {
            return Err(Error::Unauthenticated);
        }
        let row:Option<(Uuid,Uuid)>=sqlx::query_as("UPDATE sessions s SET last_seen_at=clock_timestamp() FROM users u WHERE s.token_hash=$1 AND u.id=s.user_id AND NOT u.disabled AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND s.last_seen_at>clock_timestamp()-interval '30 minutes' RETURNING s.user_id,s.id").bind(token_hash(token)).fetch_optional(&self.pool).await.map_err(storage)?;
        row.map(|(user_id, session_id)| Actor {
            correlation_id: Uuid::now_v7(),
            user_id: user_id.into(),
            session_id: session_id.into(),
        })
        .ok_or(Error::Unauthenticated)
    }
    async fn revoke(&self, token: &str) -> Result<(), Error> {
        // Resolve immutable identity before waiting on a rotating session row. A
        // single UPDATE can recheck the new hash against a pre-rotation snapshot
        // that cannot yet see the revocation handle and silently miss the row.
        let id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM sessions WHERE token_hash=$1 UNION SELECT session_id FROM session_revocation_handles WHERE token_hash=$1")
            .bind(token_hash(token)).fetch_optional(&self.pool).await.map_err(storage)?;
        if let Some(id) = id {
            sqlx::query("UPDATE sessions SET revoked_at=clock_timestamp() WHERE id=$1")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(storage)?;
        }
        Ok(())
    }
    async fn list_sessions(&self, actor: &Actor) -> Result<Vec<SessionId>, Error> {
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM sessions WHERE user_id=$1 AND revoked_at IS NULL AND expires_at>clock_timestamp() AND last_seen_at>clock_timestamp()-interval '30 minutes' ORDER BY created_at DESC LIMIT 100").bind(actor.user_id.into_uuid()).fetch_all(&self.pool).await.map(|rows| rows.into_iter().map(SessionId::from).collect()).map_err(storage)
    }
    async fn rotate(&self, actor: &Actor, old: &str, new: &str) -> Result<(), Error> {
        let mut tx = riverqueue::database::begin_postgres(&self.pool)
            .await
            .map_err(storage)?;
        let n=sqlx::query("UPDATE sessions SET token_hash=$1 WHERE id=$2 AND user_id=$3 AND token_hash=$4 AND revoked_at IS NULL AND expires_at>clock_timestamp()").bind(token_hash(new)).bind(actor.session_id.into_uuid()).bind(actor.user_id.into_uuid()).bind(token_hash(old)).execute(&mut *tx).await.map_err(storage)?.rows_affected();
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
    async fn revoke_session(&self, actor: &Actor, session: SessionId) -> Result<(), Error> {
        let changed = sqlx::query("UPDATE sessions SET revoked_at=clock_timestamp() WHERE id=$1 AND user_id=$2 AND EXISTS(SELECT 1 FROM sessions caller JOIN users u ON u.id=caller.user_id WHERE caller.id=$3 AND caller.user_id=$2 AND caller.revoked_at IS NULL AND caller.expires_at>clock_timestamp() AND caller.last_seen_at>clock_timestamp()-interval '30 minutes' AND NOT u.disabled)")
            .bind(session.into_uuid()).bind(actor.user_id.into_uuid()).bind(actor.session_id.into_uuid()).execute(&self.pool).await.map_err(storage)?.rows_affected();
        if changed == 1 {
            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }
    async fn revoke_all(&self, actor: &Actor) -> Result<(), Error> {
        sqlx::query("UPDATE sessions SET revoked_at=clock_timestamp() WHERE user_id=$1")
            .bind(actor.user_id.into_uuid())
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }
}
