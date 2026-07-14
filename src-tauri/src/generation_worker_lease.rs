use rusqlite::types::Value;
use rusqlite::{
    params, Connection, DatabaseName, Transaction, TransactionBehavior, TransactionState,
};
use std::time::Duration;

const WORKER_LEASE_WRITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_WORKER_OWNER_ID_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkerTransitionAuthority {
    owner_id: String,
    fencing_epoch: i64,
}

impl WorkerTransitionAuthority {
    pub(crate) fn owner_id(&self) -> &str {
        &self.owner_id
    }

    pub(crate) fn fencing_epoch(&self) -> i64 {
        self.fencing_epoch
    }

    #[cfg(test)]
    pub(crate) fn for_test(owner_id: impl Into<String>, fencing_epoch: i64) -> Self {
        Self {
            owner_id: owner_id.into(),
            fencing_epoch,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkerLeaseAcquireOutcome {
    Acquired {
        authority: WorkerTransitionAuthority,
        expires: i64,
    },
    Held {
        expires: i64,
    },
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub(crate) enum WorkerLeaseError {
    #[error("generation worker lease authority was lost")]
    LeaseLost,
    #[error("generation worker owner ID is invalid")]
    InvalidOwner,
    #[error("generation worker lease timing is invalid")]
    InvalidTiming,
    #[error("generation worker transition requires an immediate write transaction")]
    InvalidTransaction,
    #[error("generation worker lease time overflowed")]
    TimeOverflow,
    #[error("generation worker lease fencing epoch is exhausted")]
    EpochExhausted,
    #[error("persisted generation worker lease data is corrupt")]
    CorruptPersistedData,
    #[error("generation worker lease database error: {message}")]
    Database { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PersistedWorkerLease {
    owner_id: Option<String>,
    fencing_epoch: i64,
    acquired_at: Option<i64>,
    heartbeat_at: Option<i64>,
    expires_at: Option<i64>,
}

impl PersistedWorkerLease {
    fn active_expires_at(&self) -> Result<Option<i64>, WorkerLeaseError> {
        match self.owner_id {
            Some(_) => self
                .expires_at
                .map(Some)
                .ok_or(WorkerLeaseError::CorruptPersistedData),
            None => Ok(None),
        }
    }

    fn matches_authority(&self, authority: &WorkerTransitionAuthority) -> bool {
        self.owner_id.as_deref() == Some(authority.owner_id.as_str())
            && self.fencing_epoch == authority.fencing_epoch
    }
}

fn database_error(context: &str, error: impl std::fmt::Display) -> WorkerLeaseError {
    WorkerLeaseError::Database {
        message: format!("{context}: {error}"),
    }
}

fn owner_id_is_canonical(owner_id: &str) -> bool {
    !owner_id.is_empty()
        && owner_id.len() <= MAX_WORKER_OWNER_ID_BYTES
        && owner_id.trim() == owner_id
        && owner_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn validate_owner_id(owner_id: &str) -> Result<(), WorkerLeaseError> {
    if owner_id_is_canonical(owner_id) {
        Ok(())
    } else {
        Err(WorkerLeaseError::InvalidOwner)
    }
}

fn checked_expiry(now_ms: i64, ttl: Duration) -> Result<(i64, i64), WorkerLeaseError> {
    if now_ms < 0 {
        return Err(WorkerLeaseError::InvalidTiming);
    }
    let ttl_millis = ttl.as_millis();
    let ttl_ms = i64::try_from(ttl_millis).map_err(|_| WorkerLeaseError::TimeOverflow)?;
    if ttl_ms < 1 || !ttl.subsec_nanos().is_multiple_of(1_000_000) {
        return Err(WorkerLeaseError::InvalidTiming);
    }
    let expires_at = now_ms
        .checked_add(ttl_ms)
        .ok_or(WorkerLeaseError::TimeOverflow)?;
    Ok((ttl_ms, expires_at))
}

fn begin_immediate(conn: &Connection) -> Result<Transaction<'_>, WorkerLeaseError> {
    conn.busy_timeout(WORKER_LEASE_WRITE_BUSY_TIMEOUT)
        .map_err(|error| database_error("configure worker lease writer wait", error))?;
    Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
        .map_err(|error| database_error("begin worker lease transaction", error))
}

fn value_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Integer(value) => Some(*value),
        _ => None,
    }
}

fn value_as_optional_i64(value: &Value) -> Option<Option<i64>> {
    match value {
        Value::Null => Some(None),
        Value::Integer(value) => Some(Some(*value)),
        _ => None,
    }
}

fn value_as_optional_string(value: &Value) -> Option<Option<String>> {
    match value {
        Value::Null => Some(None),
        Value::Text(value) => Some(Some(value.clone())),
        _ => None,
    }
}

fn load_worker_lease(conn: &Connection) -> Result<PersistedWorkerLease, WorkerLeaseError> {
    let mut statement = conn
        .prepare(
            "SELECT id, owner_id, fencing_epoch, acquired_at, heartbeat_at, expires_at
               FROM generation_worker_lease",
        )
        .map_err(|error| database_error("prepare worker lease read", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| database_error("query worker lease", error))?;
    let Some(row) = rows
        .next()
        .map_err(|error| database_error("read worker lease row", error))?
    else {
        return Err(WorkerLeaseError::CorruptPersistedData);
    };
    let values = (0..6)
        .map(|index| row.get::<_, Value>(index))
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| database_error("decode worker lease row", error))?;
    if rows
        .next()
        .map_err(|error| database_error("read extra worker lease row", error))?
        .is_some()
    {
        return Err(WorkerLeaseError::CorruptPersistedData);
    }

    let id = value_as_i64(&values[0]).ok_or(WorkerLeaseError::CorruptPersistedData)?;
    let owner_id =
        value_as_optional_string(&values[1]).ok_or(WorkerLeaseError::CorruptPersistedData)?;
    let fencing_epoch = value_as_i64(&values[2]).ok_or(WorkerLeaseError::CorruptPersistedData)?;
    let acquired_at =
        value_as_optional_i64(&values[3]).ok_or(WorkerLeaseError::CorruptPersistedData)?;
    let heartbeat_at =
        value_as_optional_i64(&values[4]).ok_or(WorkerLeaseError::CorruptPersistedData)?;
    let expires_at =
        value_as_optional_i64(&values[5]).ok_or(WorkerLeaseError::CorruptPersistedData)?;

    if id != 1 || fencing_epoch < 0 {
        return Err(WorkerLeaseError::CorruptPersistedData);
    }
    match (&owner_id, acquired_at, heartbeat_at, expires_at) {
        (None, None, None, None) => {}
        (Some(owner), Some(acquired), Some(heartbeat), Some(expires))
            if owner_id_is_canonical(owner)
                && fencing_epoch > 0
                && acquired >= 0
                && acquired <= heartbeat
                && heartbeat < expires => {}
        _ => return Err(WorkerLeaseError::CorruptPersistedData),
    }

    Ok(PersistedWorkerLease {
        owner_id,
        fencing_epoch,
        acquired_at,
        heartbeat_at,
        expires_at,
    })
}

fn commit(tx: Transaction<'_>, context: &str) -> Result<(), WorkerLeaseError> {
    tx.commit().map_err(|error| database_error(context, error))
}

pub(crate) fn acquire_worker_lease(
    conn: &Connection,
    owner_id: &str,
    now_ms: i64,
    ttl: Duration,
) -> Result<WorkerLeaseAcquireOutcome, WorkerLeaseError> {
    acquire_worker_lease_with_transaction_time(conn, owner_id, ttl, || now_ms)
}

/// Acquires the immediate write transaction before sampling time so a writer
/// wait cannot make lease decisions with a pre-lock timestamp. The callback
/// must be side-effect free and is invoked exactly once after BEGIN succeeds.
pub(crate) fn acquire_worker_lease_with_transaction_time<Now>(
    conn: &Connection,
    owner_id: &str,
    ttl: Duration,
    now: Now,
) -> Result<WorkerLeaseAcquireOutcome, WorkerLeaseError>
where
    Now: FnOnce() -> i64,
{
    validate_owner_id(owner_id)?;
    let tx = begin_immediate(conn)?;
    let now_ms = now();
    let (_, expires_at) = checked_expiry(now_ms, ttl)?;
    let current = load_worker_lease(&tx)?;

    if let Some(current_expires_at) = current.active_expires_at()? {
        if now_ms < current_expires_at {
            commit(tx, "commit held worker lease observation")?;
            return Ok(WorkerLeaseAcquireOutcome::Held {
                expires: current_expires_at,
            });
        }
    }

    let next_epoch = current
        .fencing_epoch
        .checked_add(1)
        .ok_or(WorkerLeaseError::EpochExhausted)?;
    let changed = tx
        .execute(
            "UPDATE generation_worker_lease
                SET owner_id = ?1,
                    fencing_epoch = ?2,
                    acquired_at = ?3,
                    heartbeat_at = ?3,
                    expires_at = ?4
              WHERE id = 1
                AND fencing_epoch = ?5
                AND owner_id IS ?6
                AND acquired_at IS ?7
                AND heartbeat_at IS ?8
                AND expires_at IS ?9
                AND (owner_id IS NULL OR expires_at <= ?3)",
            params![
                owner_id,
                next_epoch,
                now_ms,
                expires_at,
                current.fencing_epoch,
                current.owner_id,
                current.acquired_at,
                current.heartbeat_at,
                current.expires_at,
            ],
        )
        .map_err(|error| database_error("acquire worker lease", error))?;
    if changed != 1 {
        let observed = load_worker_lease(&tx)?;
        if let Some(observed_expires_at) = observed.active_expires_at()? {
            if now_ms < observed_expires_at {
                return Err(WorkerLeaseError::LeaseLost);
            }
        }
        return Err(WorkerLeaseError::CorruptPersistedData);
    }
    let persisted = load_worker_lease(&tx)?;
    if persisted.owner_id.as_deref() != Some(owner_id)
        || persisted.fencing_epoch != next_epoch
        || persisted.acquired_at != Some(now_ms)
        || persisted.heartbeat_at != Some(now_ms)
        || persisted.expires_at != Some(expires_at)
    {
        return Err(WorkerLeaseError::CorruptPersistedData);
    }
    commit(tx, "commit worker lease acquisition")?;
    Ok(WorkerLeaseAcquireOutcome::Acquired {
        authority: WorkerTransitionAuthority {
            owner_id: owner_id.to_string(),
            fencing_epoch: next_epoch,
        },
        expires: expires_at,
    })
}

pub(crate) fn renew_worker_lease(
    conn: &Connection,
    authority: &WorkerTransitionAuthority,
    now_ms: i64,
    ttl: Duration,
) -> Result<i64, WorkerLeaseError> {
    renew_worker_lease_with_transaction_time(conn, authority, ttl, || now_ms)
}

/// Renews only after the immediate write transaction is held, then samples
/// the authority time once inside that transaction.
pub(crate) fn renew_worker_lease_with_transaction_time<Now>(
    conn: &Connection,
    authority: &WorkerTransitionAuthority,
    ttl: Duration,
    now: Now,
) -> Result<i64, WorkerLeaseError>
where
    Now: FnOnce() -> i64,
{
    validate_owner_id(&authority.owner_id)?;
    let tx = begin_immediate(conn)?;
    let now_ms = now();
    let (_, next_expires_at) = checked_expiry(now_ms, ttl)?;
    let current = load_worker_lease(&tx)?;
    if !current.matches_authority(authority) {
        return Err(WorkerLeaseError::LeaseLost);
    }
    let heartbeat_at = current
        .heartbeat_at
        .ok_or(WorkerLeaseError::CorruptPersistedData)?;
    let expires_at = current
        .expires_at
        .ok_or(WorkerLeaseError::CorruptPersistedData)?;
    if now_ms < heartbeat_at {
        return Err(WorkerLeaseError::LeaseLost);
    }
    if now_ms >= expires_at {
        return Err(WorkerLeaseError::LeaseLost);
    }
    if next_expires_at < expires_at {
        return Err(WorkerLeaseError::InvalidTiming);
    }

    let changed = tx
        .execute(
            "UPDATE generation_worker_lease
                SET heartbeat_at = ?1, expires_at = ?2
              WHERE id = 1
                AND owner_id = ?3
                AND fencing_epoch = ?4
                AND acquired_at = ?5
                AND heartbeat_at = ?6
                AND expires_at = ?7
                AND heartbeat_at <= ?1
                AND expires_at > ?1",
            params![
                now_ms,
                next_expires_at,
                authority.owner_id,
                authority.fencing_epoch,
                current.acquired_at,
                heartbeat_at,
                expires_at,
            ],
        )
        .map_err(|error| database_error("renew worker lease", error))?;
    if changed != 1 {
        let observed = load_worker_lease(&tx)?;
        if !observed.matches_authority(authority)
            || observed.expires_at.is_some_and(|expires| now_ms >= expires)
        {
            return Err(WorkerLeaseError::LeaseLost);
        }
        return Err(WorkerLeaseError::CorruptPersistedData);
    }
    commit(tx, "commit worker lease renewal")?;
    Ok(next_expires_at)
}

pub(crate) fn release_worker_lease(
    conn: &Connection,
    authority: &WorkerTransitionAuthority,
) -> Result<(), WorkerLeaseError> {
    validate_owner_id(&authority.owner_id)?;
    let tx = begin_immediate(conn)?;
    let current = load_worker_lease(&tx)?;
    if !current.matches_authority(authority) {
        return Err(WorkerLeaseError::LeaseLost);
    }
    let changed = tx
        .execute(
            "UPDATE generation_worker_lease
                SET owner_id = NULL,
                    acquired_at = NULL,
                    heartbeat_at = NULL,
                    expires_at = NULL
              WHERE id = 1
                AND owner_id = ?1
                AND fencing_epoch = ?2
                AND acquired_at = ?3
                AND heartbeat_at = ?4
                AND expires_at = ?5",
            params![
                authority.owner_id,
                authority.fencing_epoch,
                current.acquired_at,
                current.heartbeat_at,
                current.expires_at,
            ],
        )
        .map_err(|error| database_error("release worker lease", error))?;
    if changed != 1 {
        let observed = load_worker_lease(&tx)?;
        if !observed.matches_authority(authority) {
            return Err(WorkerLeaseError::LeaseLost);
        }
        return Err(WorkerLeaseError::CorruptPersistedData);
    }
    commit(tx, "commit worker lease release")
}

pub(crate) fn assert_worker_transition_authority_in_transaction(
    tx: &Transaction<'_>,
    authority: &WorkerTransitionAuthority,
    now_ms: i64,
) -> Result<(), WorkerLeaseError> {
    validate_owner_id(&authority.owner_id)?;
    if now_ms < 0 {
        return Err(WorkerLeaseError::InvalidTiming);
    }
    match tx
        .transaction_state(Some(DatabaseName::Main))
        .map_err(|error| database_error("read worker transition transaction state", error))?
    {
        TransactionState::Write => {}
        TransactionState::None | TransactionState::Read => {
            return Err(WorkerLeaseError::InvalidTransaction);
        }
        _ => return Err(WorkerLeaseError::InvalidTransaction),
    }
    let current = load_worker_lease(tx)?;
    if !current.matches_authority(authority) {
        return Err(WorkerLeaseError::LeaseLost);
    }
    let heartbeat_at = current
        .heartbeat_at
        .ok_or(WorkerLeaseError::CorruptPersistedData)?;
    let expires_at = current
        .expires_at
        .ok_or(WorkerLeaseError::CorruptPersistedData)?;
    if now_ms < heartbeat_at {
        return Err(WorkerLeaseError::LeaseLost);
    }
    if now_ms >= expires_at {
        return Err(WorkerLeaseError::LeaseLost);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use rusqlite::{
        params, Connection, DatabaseName, Transaction, TransactionBehavior, TransactionState,
    };
    use std::cell::Cell;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc, Barrier};
    use std::time::Duration;

    struct LeaseFixture {
        path: PathBuf,
        directory: PathBuf,
    }

    impl LeaseFixture {
        fn new(prefix: &str) -> Self {
            let directory = std::env::temp_dir().join(format!("{prefix}-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&directory).expect("create lease test directory");
            let path = directory.join("astro_studio.db");
            let database = Database::open(&path).expect("open lease fixture");
            database.run_migrations().expect("migrate lease fixture");
            drop(database);
            Self { path, directory }
        }

        fn open(&self) -> Connection {
            open_test_connection(&self.path)
        }
    }

    impl Drop for LeaseFixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.directory).ok();
        }
    }

    fn open_test_connection(path: &Path) -> Connection {
        let conn = Connection::open(path).expect("open lease connection");
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .expect("configure lease connection");
        conn
    }

    fn acquired(outcome: WorkerLeaseAcquireOutcome) -> (WorkerTransitionAuthority, i64) {
        match outcome {
            WorkerLeaseAcquireOutcome::Acquired { authority, expires } => (authority, expires),
            WorkerLeaseAcquireOutcome::Held { expires } => {
                panic!("expected acquired lease, held until {expires}")
            }
        }
    }

    fn row(conn: &Connection) -> (Option<String>, i64, Option<i64>, Option<i64>, Option<i64>) {
        conn.query_row(
            "SELECT owner_id, fencing_epoch, acquired_at, heartbeat_at, expires_at
               FROM generation_worker_lease",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("read lease row")
    }

    fn assert_lease_lost<T>(result: Result<T, WorkerLeaseError>) {
        assert!(matches!(result, Err(WorkerLeaseError::LeaseLost)));
    }

    #[test]
    fn transaction_time_callbacks_run_after_the_immediate_write_transaction_begins() {
        let fixture = LeaseFixture::new("astro-studio-worker-lease-transaction-time-test");
        let conn = fixture.open();
        let acquisition_sampled = Cell::new(false);
        let (authority, expires) = acquired(
            acquire_worker_lease_with_transaction_time(
                &conn,
                "transaction-clock-worker",
                Duration::from_millis(100),
                || {
                    // Test-only state inspection proves the callback runs
                    // after BEGIN; production clock callbacks are pure.
                    assert_eq!(
                        conn.transaction_state(Some(DatabaseName::Main))
                            .expect("read acquisition transaction state"),
                        TransactionState::Write
                    );
                    acquisition_sampled.set(true);
                    1_000
                },
            )
            .expect("acquire with transaction time"),
        );
        assert!(acquisition_sampled.get());
        assert_eq!(expires, 1_100);

        let renewal_sampled = Cell::new(false);
        assert_eq!(
            renew_worker_lease_with_transaction_time(
                &conn,
                &authority,
                Duration::from_millis(100),
                || {
                    // Test-only state inspection proves the callback runs
                    // after BEGIN; production clock callbacks are pure.
                    assert_eq!(
                        conn.transaction_state(Some(DatabaseName::Main))
                            .expect("read renewal transaction state"),
                        TransactionState::Write
                    );
                    renewal_sampled.set(true);
                    1_050
                },
            )
            .expect("renew with transaction time"),
            1_150
        );
        assert!(renewal_sampled.get());
    }

    #[test]
    fn acquire_samples_time_after_waiting_for_another_writer() {
        let fixture = LeaseFixture::new("astro-studio-worker-lease-acquire-clock-race-test");
        let setup = fixture.open();
        let (_, first_expiry) = acquired(
            acquire_worker_lease(&setup, "worker-a", 1_000, Duration::from_millis(100))
                .expect("acquire initial lease"),
        );
        assert_eq!(first_expiry, 1_100);

        let blocker = fixture.open();
        let blocker_tx = Transaction::new_unchecked(&blocker, TransactionBehavior::Immediate)
            .expect("hold competing writer transaction");
        let barrier = Arc::new(Barrier::new(2));
        let now_ms = Arc::new(AtomicI64::new(1_099));
        let calls = Arc::new(AtomicUsize::new(0));
        let (sampled_tx, sampled_rx) = mpsc::channel();
        let path = fixture.path.clone();
        let worker_barrier = Arc::clone(&barrier);
        let worker_now_ms = Arc::clone(&now_ms);
        let worker_calls = Arc::clone(&calls);
        let handle = std::thread::spawn(move || {
            let conn = open_test_connection(&path);
            worker_barrier.wait();
            acquire_worker_lease_with_transaction_time(
                &conn,
                "worker-b",
                Duration::from_millis(100),
                || {
                    worker_calls.fetch_add(1, Ordering::SeqCst);
                    sampled_tx.send(()).expect("signal acquire clock sample");
                    worker_now_ms.load(Ordering::SeqCst)
                },
            )
        });

        barrier.wait();
        assert!(matches!(
            sampled_rx.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        now_ms.store(1_100, Ordering::SeqCst);
        blocker_tx.commit().expect("release competing writer");
        let (authority, expires) = acquired(
            handle
                .join()
                .expect("acquire clock thread panicked")
                .expect("acquire after writer wait"),
        );
        assert_eq!(authority.fencing_epoch(), 2);
        assert_eq!(expires, 1_200);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn renew_rejects_expiry_crossed_while_waiting_for_another_writer() {
        let fixture = LeaseFixture::new("astro-studio-worker-lease-renew-clock-race-test");
        let setup = fixture.open();
        let (authority, _) = acquired(
            acquire_worker_lease(&setup, "worker-a", 1_000, Duration::from_millis(100))
                .expect("acquire renewable lease"),
        );
        let baseline = row(&setup);

        let blocker = fixture.open();
        let blocker_tx = Transaction::new_unchecked(&blocker, TransactionBehavior::Immediate)
            .expect("hold competing renewal writer");
        let barrier = Arc::new(Barrier::new(2));
        let now_ms = Arc::new(AtomicI64::new(1_099));
        let calls = Arc::new(AtomicUsize::new(0));
        let (sampled_tx, sampled_rx) = mpsc::channel();
        let path = fixture.path.clone();
        let worker_barrier = Arc::clone(&barrier);
        let worker_now_ms = Arc::clone(&now_ms);
        let worker_calls = Arc::clone(&calls);
        let handle = std::thread::spawn(move || {
            let conn = open_test_connection(&path);
            worker_barrier.wait();
            renew_worker_lease_with_transaction_time(
                &conn,
                &authority,
                Duration::from_millis(100),
                || {
                    worker_calls.fetch_add(1, Ordering::SeqCst);
                    sampled_tx.send(()).expect("signal renewal clock sample");
                    worker_now_ms.load(Ordering::SeqCst)
                },
            )
        });

        barrier.wait();
        assert!(matches!(
            sampled_rx.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        now_ms.store(1_100, Ordering::SeqCst);
        blocker_tx
            .commit()
            .expect("release competing renewal writer");
        assert_lease_lost(handle.join().expect("renewal clock thread panicked"));
        assert_eq!(row(&setup), baseline);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn worker_lease_full_two_connection_fencing_chain() {
        let fixture = LeaseFixture::new("astro-studio-worker-lease-chain-test");
        let a = fixture.open();
        let b = fixture.open();

        let (authority_a, expires_a) = acquired(
            acquire_worker_lease(&a, "worker-a", 1000, Duration::from_millis(100))
                .expect("worker A acquire"),
        );
        assert_eq!(authority_a.owner_id(), "worker-a");
        assert_eq!(authority_a.fencing_epoch(), 1);
        assert_eq!(expires_a, 1100);
        assert!(matches!(
            acquire_worker_lease(&a, "worker-a", 1001, Duration::from_millis(100))
                .expect("active owner must not reacquire or mint another authority"),
            WorkerLeaseAcquireOutcome::Held { expires: 1100 }
        ));
        assert!(matches!(
            acquire_worker_lease(&b, "worker-b", 1099, Duration::from_millis(100))
                .expect("worker B observes active lease"),
            WorkerLeaseAcquireOutcome::Held { expires: 1100 }
        ));

        let (authority_b, expires_b) = acquired(
            acquire_worker_lease(&b, "worker-b", 1100, Duration::from_millis(100))
                .expect("worker B takeover at expiry"),
        );
        assert_eq!(authority_b.fencing_epoch(), 2);
        assert_eq!(expires_b, 1200);
        let b_row = row(&b);

        assert_lease_lost(renew_worker_lease(
            &a,
            &authority_a,
            1100,
            Duration::from_millis(100),
        ));
        assert_eq!(row(&a), b_row);
        assert_lease_lost(release_worker_lease(&a, &authority_a));
        assert_eq!(row(&a), b_row);
        {
            let tx = Transaction::new_unchecked(&a, TransactionBehavior::Immediate)
                .expect("begin stale authority assertion");
            assert_lease_lost(assert_worker_transition_authority_in_transaction(
                &tx,
                &authority_a,
                1100,
            ));
            tx.rollback().expect("rollback stale assertion");
        }
        assert_eq!(row(&a), b_row);

        {
            let tx = Transaction::new_unchecked(&b, TransactionBehavior::Immediate)
                .expect("begin current authority assertion");
            assert_worker_transition_authority_in_transaction(&tx, &authority_b, 1100)
                .expect("worker B remains authoritative");
            tx.commit().expect("commit authority-only transaction");
        }
        assert_eq!(
            renew_worker_lease(&b, &authority_b, 1150, Duration::from_millis(100),)
                .expect("renew worker B"),
            1250
        );
        release_worker_lease(&b, &authority_b).expect("release worker B");
        assert_eq!(row(&b), (None, 2, None, None, None));

        let (authority_c, expires_c) = acquired(
            acquire_worker_lease(&a, "worker-c", 1250, Duration::from_millis(100))
                .expect("worker C acquire after release"),
        );
        assert_eq!(authority_c.fencing_epoch(), 3);
        assert_eq!(expires_c, 1350);
    }

    #[test]
    fn worker_lease_concurrent_acquire_has_exactly_one_winner() {
        let fixture = LeaseFixture::new("astro-studio-worker-lease-concurrent-test");
        let barrier = Arc::new(Barrier::new(3));
        let paths = [fixture.path.clone(), fixture.path.clone()];
        let handles = paths
            .into_iter()
            .enumerate()
            .map(|(index, path)| {
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let conn = open_test_connection(&path);
                    barrier.wait();
                    acquire_worker_lease(
                        &conn,
                        &format!("worker-{index}"),
                        1000,
                        Duration::from_millis(100),
                    )
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let outcomes = handles
            .into_iter()
            .map(|handle| handle.join().expect("acquire thread panicked"))
            .collect::<Result<Vec<_>, _>>()
            .expect("concurrent acquire returned an error");

        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(outcome, WorkerLeaseAcquireOutcome::Acquired { .. }))
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(outcome, WorkerLeaseAcquireOutcome::Held { .. }))
                .count(),
            1
        );
        assert!(outcomes.iter().all(|outcome| match outcome {
            WorkerLeaseAcquireOutcome::Acquired { expires, .. }
            | WorkerLeaseAcquireOutcome::Held { expires } => *expires == 1100,
        }));
    }

    #[test]
    fn worker_lease_concurrent_expiry_takeover_mints_one_next_epoch_authority() {
        let fixture = LeaseFixture::new("astro-studio-worker-lease-takeover-race-test");
        let setup = fixture.open();
        let (authority_a, expires_a) = acquired(
            acquire_worker_lease(&setup, "worker-a", 1000, Duration::from_millis(100))
                .expect("acquire active worker A lease"),
        );
        assert_eq!(authority_a.fencing_epoch(), 1);
        assert_eq!(expires_a, 1100);
        drop(setup);

        let barrier = Arc::new(Barrier::new(3));
        let handles = ["worker-b", "worker-c"]
            .into_iter()
            .map(|owner_id| {
                let barrier = Arc::clone(&barrier);
                let path = fixture.path.clone();
                std::thread::spawn(move || {
                    let conn = open_test_connection(&path);
                    barrier.wait();
                    let outcome =
                        acquire_worker_lease(&conn, owner_id, 1100, Duration::from_millis(100));
                    (owner_id, outcome)
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let outcomes = handles
            .into_iter()
            .map(|handle| handle.join().expect("takeover thread panicked"))
            .collect::<Vec<_>>();

        let winners = outcomes
            .iter()
            .filter_map(|(owner_id, outcome)| match outcome {
                Ok(WorkerLeaseAcquireOutcome::Acquired { authority, expires }) => {
                    assert_eq!(authority.owner_id(), *owner_id);
                    assert_eq!(authority.fencing_epoch(), 2);
                    assert_eq!(*expires, 1200);
                    Some(*owner_id)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(winners.len(), 1);
        assert_eq!(
            outcomes
                .iter()
                .filter(|(_, outcome)| matches!(
                    outcome,
                    Ok(WorkerLeaseAcquireOutcome::Held { expires: 1200 })
                ))
                .count(),
            1
        );
        assert!(outcomes.iter().all(|(_, outcome)| outcome.is_ok()));

        let conn = fixture.open();
        assert_eq!(
            row(&conn),
            (
                Some(winners[0].to_string()),
                2,
                Some(1100),
                Some(1100),
                Some(1200)
            )
        );
    }

    #[test]
    fn worker_lease_rejects_invalid_owner_time_and_ttl_inputs() {
        let fixture = LeaseFixture::new("astro-studio-worker-lease-input-test");
        let conn = fixture.open();

        for owner in [
            "",
            " worker-a",
            "worker-a ",
            "worker\nname",
            &"x".repeat(129),
        ] {
            assert!(matches!(
                acquire_worker_lease(&conn, owner, 1000, Duration::from_millis(100)),
                Err(WorkerLeaseError::InvalidOwner)
            ));
        }
        for ttl in [
            Duration::ZERO,
            Duration::from_nanos(999_999),
            Duration::from_micros(1500),
        ] {
            assert!(matches!(
                acquire_worker_lease(&conn, "worker-a", 1000, ttl),
                Err(WorkerLeaseError::InvalidTiming)
            ));
        }
        assert!(matches!(
            acquire_worker_lease(&conn, "worker-a", -1, Duration::from_millis(100)),
            Err(WorkerLeaseError::InvalidTiming)
        ));
        assert!(matches!(
            acquire_worker_lease(&conn, "worker-a", i64::MAX, Duration::from_millis(1)),
            Err(WorkerLeaseError::TimeOverflow)
        ));
        assert!(matches!(
            acquire_worker_lease(&conn, "worker-a", 0, Duration::MAX),
            Err(WorkerLeaseError::TimeOverflow)
        ));
    }

    #[test]
    fn worker_lease_renew_rejects_clock_rollback_expiry_and_shortening() {
        let fixture = LeaseFixture::new("astro-studio-worker-lease-renew-time-test");
        let conn = fixture.open();
        let (authority, _) = acquired(
            acquire_worker_lease(&conn, "worker-a", 1000, Duration::from_millis(100))
                .expect("acquire lease"),
        );
        let expected = row(&conn);

        assert!(matches!(
            renew_worker_lease(&conn, &authority, 999, Duration::from_millis(200)),
            Err(WorkerLeaseError::LeaseLost)
        ));
        assert!(matches!(
            renew_worker_lease(&conn, &authority, 1050, Duration::from_millis(1)),
            Err(WorkerLeaseError::InvalidTiming)
        ));
        assert_lease_lost(renew_worker_lease(
            &conn,
            &authority,
            1100,
            Duration::from_millis(100),
        ));
        assert_eq!(row(&conn), expected);

        renew_worker_lease(&conn, &authority, 1050, Duration::from_millis(100))
            .expect("advance heartbeat for authority assertion checks");
        let tx = Transaction::new_unchecked(&conn, TransactionBehavior::Immediate)
            .expect("begin authority timing checks");
        assert_lease_lost(assert_worker_transition_authority_in_transaction(
            &tx, &authority, 1049,
        ));
        assert!(matches!(
            assert_worker_transition_authority_in_transaction(&tx, &authority, -1),
            Err(WorkerLeaseError::InvalidTiming)
        ));
        tx.rollback().expect("rollback authority timing checks");
    }

    #[test]
    fn worker_lease_reaches_then_reports_epoch_exhaustion_without_sql_increment() {
        let fixture = LeaseFixture::new("astro-studio-worker-lease-epoch-test");
        let conn = fixture.open();
        conn.execute(
            "DROP TRIGGER enforce_generation_worker_lease_transition",
            [],
        )
        .expect("drop transition trigger for max epoch fixture");
        conn.execute(
            "UPDATE generation_worker_lease SET fencing_epoch = ?1 WHERE id = 1",
            params![i64::MAX - 1],
        )
        .expect("seed penultimate fencing epoch");

        let (final_authority, final_expiry) = acquired(
            acquire_worker_lease(&conn, "worker-final", 1000, Duration::from_millis(100))
                .expect("acquire final representable fencing epoch"),
        );
        assert_eq!(final_authority.fencing_epoch(), i64::MAX);
        assert_eq!(final_expiry, 1100);
        release_worker_lease(&conn, &final_authority).expect("release final fencing epoch");

        assert!(matches!(
            acquire_worker_lease(&conn, "worker-a", 1000, Duration::from_millis(100)),
            Err(WorkerLeaseError::EpochExhausted)
        ));
        assert_eq!(row(&conn), (None, i64::MAX, None, None, None));
    }

    #[test]
    fn worker_lease_exact_authority_can_release_after_expiry_before_takeover() {
        let fixture = LeaseFixture::new("astro-studio-worker-lease-expired-release-test");
        let conn = fixture.open();
        let (authority, _) = acquired(
            acquire_worker_lease(&conn, "worker-a", 1000, Duration::from_millis(1))
                .expect("acquire short lease"),
        );
        {
            let tx = Transaction::new_unchecked(&conn, TransactionBehavior::Immediate)
                .expect("begin expired authority check");
            assert_lease_lost(assert_worker_transition_authority_in_transaction(
                &tx, &authority, 1001,
            ));
            tx.rollback().expect("rollback expired authority check");
        }

        release_worker_lease(&conn, &authority)
            .expect("exact authority may release after expiry before takeover");
        assert_eq!(row(&conn), (None, 1, None, None, None));
    }

    #[test]
    fn authority_assert_rejects_deferred_transactions_before_or_after_a_read() {
        let fixture = LeaseFixture::new("astro-studio-worker-lease-deferred-assert-test");
        let setup = fixture.open();
        let (authority, _) = acquired(
            acquire_worker_lease(&setup, "worker-a", 1000, Duration::from_millis(100))
                .expect("acquire authority for transaction-mode checks"),
        );
        setup
            .execute(
                "INSERT INTO generations (id, prompt, status)
                 VALUES ('transaction-mode-generation', 'probe', 'pending')",
                [],
            )
            .expect("insert transaction-mode generation");
        setup
            .execute(
                "INSERT INTO generation_jobs (
                    id, client_request_id, generation_id, source_kind, status, stage,
                    request_json, provider_kind, provider_profile_id, endpoint_snapshot, queued_at
                 ) VALUES (
                    'transaction-mode-job', 'transaction-mode-request',
                    'transaction-mode-generation', 'generate', 'queued', 'queued', '{}',
                    'openai', 'default', 'https://example.test', '2026-07-13T00:00:00Z'
                 )",
                [],
            )
            .expect("insert transaction-mode job");
        let expected_lease = row(&setup);
        drop(setup);

        for read_first in [false, true] {
            let conn = fixture.open();
            let tx = Transaction::new_unchecked(&conn, TransactionBehavior::Deferred)
                .expect("begin deferred transaction");
            if read_first {
                assert_eq!(
                    tx.query_row(
                        "SELECT status FROM generation_jobs WHERE id = 'transaction-mode-job'",
                        [],
                        |row| row.get::<_, String>(0),
                    )
                    .expect("establish deferred read snapshot"),
                    "queued"
                );
            }
            assert!(matches!(
                assert_worker_transition_authority_in_transaction(&tx, &authority, 1000),
                Err(WorkerLeaseError::InvalidTransaction)
            ));
            tx.rollback()
                .expect("rollback rejected deferred transaction");
            assert_eq!(row(&conn), expected_lease);
            assert_eq!(
                conn.query_row(
                    "SELECT status FROM generation_jobs WHERE id = 'transaction-mode-job'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .expect("read unchanged job"),
                "queued"
            );
        }

        let conn = fixture.open();
        let tx = Transaction::new_unchecked(&conn, TransactionBehavior::Immediate)
            .expect("begin immediate transaction");
        assert_worker_transition_authority_in_transaction(&tx, &authority, 1000)
            .expect("immediate write transaction remains valid");
        tx.commit().expect("commit immediate authority assertion");
    }

    #[test]
    fn worker_lease_distinguishes_missing_schema_and_corrupt_rows() {
        let missing_table = Connection::open_in_memory().expect("open empty database");
        assert!(matches!(
            acquire_worker_lease(&missing_table, "worker-a", 1000, Duration::from_millis(100)),
            Err(WorkerLeaseError::Database { .. })
        ));

        let missing_fixture = LeaseFixture::new("astro-studio-worker-lease-missing-row-test");
        let missing_row = missing_fixture.open();
        missing_row
            .execute("DROP TRIGGER prevent_generation_worker_lease_delete", [])
            .expect("drop delete seal for corrupt fixture");
        missing_row
            .execute("DELETE FROM generation_worker_lease", [])
            .expect("delete singleton for corrupt fixture");
        assert!(matches!(
            acquire_worker_lease(&missing_row, "worker-a", 1000, Duration::from_millis(100)),
            Err(WorkerLeaseError::CorruptPersistedData)
        ));

        let malformed_fixture = LeaseFixture::new("astro-studio-worker-lease-malformed-test");
        let malformed = malformed_fixture.open();
        malformed
            .execute(
                "DROP TRIGGER enforce_generation_worker_lease_transition",
                [],
            )
            .expect("drop update seal for corrupt fixture");
        malformed
            .execute_batch("PRAGMA ignore_check_constraints=ON;")
            .expect("allow malformed fixture");
        malformed
            .execute(
                "UPDATE generation_worker_lease
                    SET owner_id = ' worker-a', fencing_epoch = 1,
                        acquired_at = 1000, heartbeat_at = 1000, expires_at = 1100
                  WHERE id = 1",
                [],
            )
            .expect("persist malformed owner fixture");
        assert!(matches!(
            acquire_worker_lease(&malformed, "worker-b", 1200, Duration::from_millis(100)),
            Err(WorkerLeaseError::CorruptPersistedData)
        ));
    }

    #[test]
    fn fenced_composite_transaction_rejects_stale_before_job_mutations() {
        let fixture = LeaseFixture::new("astro-studio-worker-lease-composite-test");
        let a = fixture.open();
        let b = fixture.open();
        let (authority_a, _) = acquired(
            acquire_worker_lease(&a, "worker-a", 1000, Duration::from_millis(100))
                .expect("acquire A"),
        );
        let (authority_b, _) = acquired(
            acquire_worker_lease(&b, "worker-b", 1100, Duration::from_millis(100))
                .expect("take over with B"),
        );

        b.execute(
            "INSERT INTO generations (id, prompt, status) VALUES ('fenced-generation', 'probe', 'pending')",
            [],
        )
        .expect("insert fenced generation");
        b.execute(
            "INSERT INTO generation_jobs (
                id, client_request_id, generation_id, source_kind, status, stage, request_json,
                provider_kind, provider_profile_id, endpoint_snapshot, queued_at
             ) VALUES (
                'fenced-job', 'fenced-request', 'fenced-generation', 'generate', 'queued',
                'queued', '{}', 'openai', 'default', 'https://example.test',
                '2026-07-13T00:00:00Z'
             )",
            [],
        )
        .expect("insert fenced job");

        assert_lease_lost(fenced_job_update(&a, &authority_a, 1100));
        assert_eq!(
            job_and_generation_status(&a),
            ("queued".into(), "pending".into())
        );

        fenced_job_update(&b, &authority_b, 1100).expect("current worker commits both writes");
        assert_eq!(
            job_and_generation_status(&b),
            ("running".into(), "processing".into())
        );
    }

    fn fenced_job_update(
        conn: &Connection,
        authority: &WorkerTransitionAuthority,
        now_ms: i64,
    ) -> Result<(), WorkerLeaseError> {
        let tx =
            Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(|error| {
                WorkerLeaseError::Database {
                    message: error.to_string(),
                }
            })?;
        assert_worker_transition_authority_in_transaction(&tx, authority, now_ms)?;
        tx.execute(
            "UPDATE generation_jobs SET status = 'running', stage = 'preparing' WHERE id = 'fenced-job'",
            [],
        )
        .map_err(|error| WorkerLeaseError::Database {
            message: error.to_string(),
        })?;
        tx.execute(
            "UPDATE generations SET status = 'processing' WHERE id = 'fenced-generation'",
            [],
        )
        .map_err(|error| WorkerLeaseError::Database {
            message: error.to_string(),
        })?;
        tx.commit().map_err(|error| WorkerLeaseError::Database {
            message: error.to_string(),
        })
    }

    fn job_and_generation_status(conn: &Connection) -> (String, String) {
        conn.query_row(
            "SELECT j.status, g.status
               FROM generation_jobs j
               JOIN generations g ON g.id = j.generation_id
              WHERE j.id = 'fenced-job'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read fenced job and generation")
    }
}
