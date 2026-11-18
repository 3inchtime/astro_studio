CREATE TRIGGER IF NOT EXISTS prevent_generation_worker_lease_epoch_decrease
BEFORE UPDATE OF fencing_epoch ON generation_worker_lease
WHEN NEW.fencing_epoch < OLD.fencing_epoch
BEGIN
    SELECT RAISE(ABORT, 'generation worker fencing epoch decreased');
END;
