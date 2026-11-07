use crate::Database;
use application::{Actor, IdentityRepository, Secrets};
use async_trait::async_trait;
use domain::Error;
use rand::RngExt;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub struct SystemSecrets;
impl Secrets for SystemSecrets {
    fn token(&self) -> String {
        hex::encode(rand::rng().random::<[u8; 32]>())
    }
}
pub fn token_hash(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}
fn storage(_: sqlx::Error) -> Error {
    Error::Unavailable
}

#[async_trait]
impl IdentityRepository for Database {
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
            user_id,
            session_id,
        })
    }
    async fn authenticate(&self, token: &str) -> Result<Actor, Error> {
        if token.len() != 64 {
            return Err(Error::Unauthenticated);
        }
        let row:Option<(Uuid,Uuid)>=sqlx::query_as("UPDATE sessions s SET last_seen_at=clock_timestamp() FROM users u WHERE s.token_hash=$1 AND u.id=s.user_id AND NOT u.disabled AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND s.last_seen_at>clock_timestamp()-interval '30 minutes' RETURNING s.user_id,s.id").bind(token_hash(token)).fetch_optional(&self.pool).await.map_err(storage)?;
        row.map(|(user_id, session_id)| Actor {
            user_id,
            session_id,
        })
        .ok_or(Error::Unauthenticated)
    }
    async fn revoke(&self, token: &str) -> Result<(), Error> {
        sqlx::query("UPDATE sessions SET revoked_at=clock_timestamp() WHERE token_hash=$1 OR id IN (SELECT session_id FROM session_revocation_handles WHERE token_hash=$1)").bind(token_hash(token)).execute(&self.pool).await.map_err(storage)?;
        Ok(())
    }
    async fn list_sessions(&self, actor: &Actor) -> Result<Vec<Uuid>, Error> {
        sqlx::query_scalar("SELECT id FROM sessions WHERE user_id=$1 AND revoked_at IS NULL AND expires_at>clock_timestamp() ORDER BY created_at DESC LIMIT 100").bind(actor.user_id).fetch_all(&self.pool).await.map_err(storage)
    }
    async fn rotate(&self, actor: &Actor, old: &str, new: &str) -> Result<(), Error> {
        let mut tx = riverqueue::database::begin_postgres(&self.pool)
            .await
            .map_err(storage)?;
        let n=sqlx::query("UPDATE sessions SET token_hash=$1 WHERE id=$2 AND user_id=$3 AND token_hash=$4 AND revoked_at IS NULL AND expires_at>clock_timestamp()").bind(token_hash(new)).bind(actor.session_id).bind(actor.user_id).bind(token_hash(old)).execute(&mut *tx).await.map_err(storage)?.rows_affected();
        if n != 1 {
            return Err(Error::Unauthenticated);
        }
        sqlx::query("INSERT INTO session_revocation_handles(token_hash,session_id) VALUES($1,$2)")
            .bind(token_hash(old))
            .bind(actor.session_id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)
    }
    async fn revoke_all(&self, actor: &Actor) -> Result<(), Error> {
        sqlx::query("UPDATE sessions SET revoked_at=clock_timestamp() WHERE user_id=$1")
            .bind(actor.user_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }
}
