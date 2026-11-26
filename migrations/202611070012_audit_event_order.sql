CREATE INDEX audit_events_chronological ON audit_events (organization_id, occurred_at DESC, id DESC);
DROP INDEX audit_events_tenant;
