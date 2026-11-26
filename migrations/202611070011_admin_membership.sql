-- SQLx runs this cutover in one transaction. The table rewrites retain NOT NULL,
-- tenant policies, and runtime grants; no runtime role needs to bypass RLS.
ALTER TABLE memberships
 DROP CONSTRAINT memberships_role_check,
 ALTER COLUMN role TYPE boolean USING (role IN ('owner','admin'));
ALTER TABLE memberships RENAME COLUMN role TO is_admin;

ALTER TABLE invitations
 DROP CONSTRAINT invitations_role_check,
 ALTER COLUMN role TYPE boolean USING (role IN ('owner','admin'));
ALTER TABLE invitations RENAME COLUMN role TO is_admin;

DROP TABLE reauthentication_proofs;
