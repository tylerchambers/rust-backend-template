CREATE TABLE magic_links (
 token_hash bytea PRIMARY KEY CHECK(octet_length(token_hash)=32),
 email text NOT NULL,
 expires_at timestamptz NOT NULL DEFAULT (clock_timestamp()+interval '10 minutes'),
 consumed_at timestamptz
);
CREATE TABLE sessions (
 id uuid PRIMARY KEY,
 user_id uuid NOT NULL REFERENCES users(id),
 token_hash bytea NOT NULL UNIQUE CHECK(octet_length(token_hash)=32),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 last_seen_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 expires_at timestamptz NOT NULL DEFAULT (clock_timestamp()+interval '7 days'),
 revoked_at timestamptz
);
CREATE INDEX sessions_user ON sessions(user_id);
CREATE TABLE auth_limits (
 key_hash bytea PRIMARY KEY,
 window_start timestamptz NOT NULL,
 attempts integer NOT NULL CHECK(attempts>0)
);
GRANT SELECT,INSERT,UPDATE,DELETE ON magic_links,sessions,auth_limits TO template_app;
-- River owns its schema. Runtime can enqueue but cannot run DDL.
GRANT SELECT,INSERT,UPDATE,DELETE ON ALL TABLES IN SCHEMA public TO template_app;
GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA public TO template_app;
REVOKE UPDATE,DELETE ON audit_events FROM template_app;
REVOKE ALL ON _sqlx_migrations FROM template_app;
