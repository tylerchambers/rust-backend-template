-- Development fixture only; creates no authentication token or session.
BEGIN;
-- Serialize fixture publication with concurrent writes; bounded by the seed deadline.
SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '20s';
LOCK TABLE users, organizations, memberships, projects IN SHARE ROW EXCLUSIVE MODE;
INSERT INTO users(id,email) VALUES
 ('00000000-0000-7000-8000-000000000001','owner@example.test')
ON CONFLICT(id) DO NOTHING;
INSERT INTO organizations(id,name) VALUES
 ('00000000-0000-7000-8000-000000000101','Example North'),
 ('00000000-0000-7000-8000-000000000102','Example South')
ON CONFLICT(id) DO NOTHING;
INSERT INTO memberships(organization_id,user_id,is_admin) VALUES
 ('00000000-0000-7000-8000-000000000101','00000000-0000-7000-8000-000000000001',true),
 ('00000000-0000-7000-8000-000000000102','00000000-0000-7000-8000-000000000001',true)
ON CONFLICT(organization_id,user_id) DO NOTHING;
INSERT INTO projects(id,organization_id,name) VALUES
 ('00000000-0000-7000-8000-000000001001','00000000-0000-7000-8000-000000000101','North example'),
 ('00000000-0000-7000-8000-000000001002','00000000-0000-7000-8000-000000000102','South example')
ON CONFLICT(id) DO NOTHING;
-- Validate before commit: a conflict rolls back every insert.
DO $$
BEGIN
 IF NOT EXISTS (
  SELECT FROM users WHERE id='00000000-0000-7000-8000-000000000001'
  AND email='owner@example.test' AND NOT disabled
 ) OR EXISTS (
  SELECT FROM (VALUES
   ('00000000-0000-7000-8000-000000000101'::uuid,'Example North'),
   ('00000000-0000-7000-8000-000000000102'::uuid,'Example South')
  ) expected(id,name)
  LEFT JOIN organizations o ON o.id=expected.id
  WHERE o.id IS NULL OR o.name<>expected.name OR o.version<>1
 ) OR EXISTS (
  SELECT FROM memberships WHERE user_id='00000000-0000-7000-8000-000000000001'
  AND organization_id IN ('00000000-0000-7000-8000-000000000101','00000000-0000-7000-8000-000000000102')
  AND (NOT is_admin OR version<>1)
 ) OR EXISTS (
  SELECT FROM (VALUES
   ('00000000-0000-7000-8000-000000001001'::uuid,'00000000-0000-7000-8000-000000000101'::uuid,'North example'),
   ('00000000-0000-7000-8000-000000001002'::uuid,'00000000-0000-7000-8000-000000000102'::uuid,'South example')
  ) expected(id,organization_id,name)
  LEFT JOIN projects p ON p.id=expected.id
  WHERE p.id IS NULL OR p.organization_id<>expected.organization_id
  OR p.name<>expected.name OR p.version<>1 OR p.archived
 ) THEN
  RAISE EXCEPTION 'Seed conflict: existing fixture identity or shape differs';
 END IF;
END $$;
COMMIT;
