-- Account-scoped discovery is SELECT-only. Normal tenant writes remain guarded
-- by the unchanged tenant policies. The application validates a live session
-- before setting this transaction-local context; it never accepts it from HTTP.
CREATE POLICY self_discovery ON memberships FOR SELECT USING (
 user_id = nullif(current_setting('app.discovery_user_id',true),'')::uuid
);
CREATE POLICY self_discovery ON organizations FOR SELECT USING (
 EXISTS (SELECT 1 FROM memberships m WHERE m.organization_id=organizations.id
 AND m.user_id=nullif(current_setting('app.discovery_user_id',true),'')::uuid)
);
