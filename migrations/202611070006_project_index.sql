CREATE TABLE project_index (
 organization_id uuid NOT NULL,
 project_id uuid PRIMARY KEY,
 name text NOT NULL,
 version bigint NOT NULL,
 FOREIGN KEY(organization_id,project_id) REFERENCES projects(organization_id,id) ON DELETE CASCADE
);
ALTER TABLE project_index ENABLE ROW LEVEL SECURITY;
ALTER TABLE project_index FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON project_index USING(organization_id=nullif(current_setting('app.organization_id',true),'')::uuid) WITH CHECK(organization_id=nullif(current_setting('app.organization_id',true),'')::uuid);
CREATE INDEX project_index_tenant ON project_index(organization_id,project_id);
GRANT SELECT,INSERT,UPDATE,DELETE ON project_index TO template_app;
