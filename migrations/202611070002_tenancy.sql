-- Provisioning role creation is deploy-owned; runtime never uses this credential.
CREATE ROLE template_app NOLOGIN NOSUPERUSER NOBYPASSRLS;
CREATE TABLE users (
 id uuid PRIMARY KEY,
 email text NOT NULL UNIQUE CHECK (email=lower(email)),
 disabled boolean NOT NULL DEFAULT false,
 verified_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE memberships (
 organization_id uuid NOT NULL REFERENCES organizations(id),
 user_id uuid NOT NULL REFERENCES users(id),
 role text NOT NULL CHECK(role IN ('owner','admin','member','viewer')),
 version bigint NOT NULL DEFAULT 1,
 PRIMARY KEY(organization_id,user_id)
);
CREATE TABLE projects (
 id uuid PRIMARY KEY,
 organization_id uuid NOT NULL REFERENCES organizations(id),
 name text NOT NULL CHECK(length(name) BETWEEN 1 AND 120),
 version bigint NOT NULL DEFAULT 1 CHECK(version>0),
 archived boolean NOT NULL DEFAULT false,
 UNIQUE(organization_id,id)
);
CREATE INDEX projects_page ON projects(organization_id,id);
CREATE TABLE audit_events (
 id uuid PRIMARY KEY,
 organization_id uuid NOT NULL REFERENCES organizations(id),
 actor_id uuid NOT NULL REFERENCES users(id),
 action text NOT NULL,
 subject uuid NOT NULL,
 occurred_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE invitations (
 id uuid PRIMARY KEY,
 organization_id uuid NOT NULL REFERENCES organizations(id),
 email text NOT NULL,
 role text NOT NULL CHECK(role IN ('owner','admin','member','viewer')),
 token_hash bytea NOT NULL UNIQUE,
 expires_at timestamptz NOT NULL,
 accepted_at timestamptz,
 revoked_at timestamptz
);
CREATE INDEX invitations_tenant ON invitations(organization_id,id);
CREATE INDEX audit_events_tenant ON audit_events(organization_id,id);
CREATE INDEX memberships_user ON memberships(user_id,organization_id);
DO $$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['organizations','memberships','projects','audit_events','invitations'] LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t);
  EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format('CREATE POLICY tenant ON %I USING (%I = nullif(current_setting(''app.organization_id'',true),'''')::uuid) WITH CHECK (%I = nullif(current_setting(''app.organization_id'',true),'''')::uuid)',t,CASE WHEN t='organizations' THEN 'id' ELSE 'organization_id' END,CASE WHEN t='organizations' THEN 'id' ELSE 'organization_id' END);
 END LOOP;
END $$;
GRANT USAGE ON SCHEMA public TO template_app;
GRANT SELECT,INSERT,UPDATE,DELETE ON organizations,memberships,projects,invitations TO template_app;
GRANT SELECT,INSERT ON audit_events TO template_app;
GRANT SELECT,INSERT,UPDATE ON users TO template_app;
