-- Repair clean installations where the first runtime grant preceded River DDL.
-- Keep River schema migrations themselves under the official migrator.
GRANT SELECT,INSERT,UPDATE,DELETE ON river_job,river_queue,river_leader,river_notification TO template_app;
GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA public TO template_app;
REVOKE INSERT,UPDATE,DELETE ON river_migration FROM template_app;
