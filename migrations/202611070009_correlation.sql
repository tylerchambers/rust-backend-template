-- Expand-only: older application revisions may omit request correlation.
ALTER TABLE audit_events ADD COLUMN correlation_id uuid;
CREATE INDEX audit_events_correlation ON audit_events(correlation_id) WHERE correlation_id IS NOT NULL;
