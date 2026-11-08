CREATE TABLE identity_audit (
 id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 user_id uuid NOT NULL,
 session_id uuid,
 action text NOT NULL CHECK(action IN ('identity.created','identity.status_changed','session.created','session.rotated','session.revoked')),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
-- Reviewed global administrative surface: identity/session events only. No email,
-- token/hash, request payload or arbitrary event data is accepted by this function.
CREATE FUNCTION audit_identity_lifecycle() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE event_action text; subject_user uuid; subject_session uuid;
BEGIN
 IF TG_TABLE_NAME='users' THEN
  subject_user:=NEW.id;
  IF TG_OP='INSERT' THEN event_action:='identity.created';
  ELSIF NEW.disabled IS DISTINCT FROM OLD.disabled THEN event_action:='identity.status_changed'; END IF;
 ELSE
  subject_user:=NEW.user_id; subject_session:=NEW.id;
  IF TG_OP='INSERT' THEN event_action:='session.created';
  ELSIF NEW.revoked_at IS DISTINCT FROM OLD.revoked_at THEN event_action:='session.revoked';
  ELSIF NEW.token_hash IS DISTINCT FROM OLD.token_hash THEN event_action:='session.rotated'; END IF;
 END IF;
 IF event_action IS NOT NULL THEN
  INSERT INTO public.identity_audit(user_id,session_id,action) VALUES(subject_user,subject_session,event_action);
 END IF;
 RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION audit_identity_lifecycle() FROM PUBLIC;
CREATE TRIGGER users_identity_audit AFTER INSERT OR UPDATE ON users FOR EACH ROW EXECUTE FUNCTION audit_identity_lifecycle();
CREATE TRIGGER sessions_identity_audit AFTER INSERT OR UPDATE ON sessions FOR EACH ROW EXECUTE FUNCTION audit_identity_lifecycle();
REVOKE ALL ON identity_audit FROM template_app;
