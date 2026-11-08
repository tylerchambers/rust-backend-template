CREATE TABLE session_revocation_handles (
 token_hash bytea PRIMARY KEY,
 session_id uuid NOT NULL REFERENCES sessions(id) ON DELETE CASCADE
);
CREATE INDEX session_revocation_session ON session_revocation_handles(session_id);
GRANT SELECT,INSERT,DELETE ON session_revocation_handles TO template_app;
