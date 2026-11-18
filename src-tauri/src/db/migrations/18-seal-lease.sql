CREATE TRIGGER prevent_generation_worker_lease_delete
BEFORE DELETE ON generation_worker_lease
BEGIN
    SELECT RAISE(ABORT, 'generation worker lease row cannot be deleted');
END;

CREATE TRIGGER prevent_generation_worker_lease_insert
BEFORE INSERT ON generation_worker_lease
BEGIN
    SELECT RAISE(ABORT, 'generation worker lease row is sealed');
END;

CREATE TRIGGER enforce_generation_worker_lease_transition
BEFORE UPDATE ON generation_worker_lease
WHEN CASE
    WHEN OLD.owner_id IS NOT NULL
         AND NEW.id = OLD.id
         AND NEW.owner_id = OLD.owner_id
         AND NEW.fencing_epoch = OLD.fencing_epoch
         AND NEW.acquired_at = OLD.acquired_at
         AND NEW.heartbeat_at >= OLD.heartbeat_at
         AND NEW.heartbeat_at < OLD.expires_at
         AND NEW.expires_at >= OLD.expires_at
    THEN 0
    WHEN OLD.owner_id IS NOT NULL
         AND NEW.id = OLD.id
         AND NEW.owner_id IS NULL
         AND NEW.fencing_epoch = OLD.fencing_epoch
         AND NEW.acquired_at IS NULL
         AND NEW.heartbeat_at IS NULL
         AND NEW.expires_at IS NULL
    THEN 0
    WHEN NEW.id = OLD.id
         AND NEW.owner_id IS NOT NULL
         AND OLD.fencing_epoch < 9223372036854775807
         AND NEW.fencing_epoch = OLD.fencing_epoch + 1
         AND NEW.acquired_at = NEW.heartbeat_at
         AND (
             (
                 OLD.owner_id IS NULL
                 AND OLD.acquired_at IS NULL
                 AND OLD.heartbeat_at IS NULL
                 AND OLD.expires_at IS NULL
             )
             OR OLD.expires_at <= NEW.acquired_at
         )
    THEN 0
    ELSE 1
END = 1
BEGIN
    SELECT RAISE(ABORT, 'invalid generation worker lease transition');
END;
