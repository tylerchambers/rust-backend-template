CREATE TABLE reauthentication_proofs (
 token_hash bytea PRIMARY KEY CHECK(octet_length(token_hash)=32),
 user_id uuid NOT NULL REFERENCES users(id),
 session_id uuid NOT NULL REFERENCES sessions(id),
 organization_id uuid NOT NULL REFERENCES organizations(id),
 action text NOT NULL CHECK(action IN ('change_role','invite','revoke_invitation','remove_member')),
 target uuid NOT NULL,
 expires_at timestamptz NOT NULL DEFAULT (clock_timestamp()+interval '5 minutes'),
 consumed_at timestamptz
);
GRANT SELECT,INSERT,UPDATE,DELETE ON reauthentication_proofs TO template_app;
