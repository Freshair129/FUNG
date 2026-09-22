//! Tier-3 (cloud) fallback policy: an atomic admission boundary plus its two
//! bits of local SQLite-backed state (the policy row, the daily call
//! counter). No secrets live here — cloud API keys stay in cloud_config.rs's
//! keyring entries; this module only decides whether cloud is *allowed*.

use crate::cloud_config::CloudTaskKind;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TierPolicy {
    pub(crate) stt_cloud_enabled: bool,
    pub(crate) llm_cloud_enabled: bool,
    pub(crate) daily_cap: u32,
}

impl Default for TierPolicy {
    fn default() -> Self {
        Self {
            stt_cloud_enabled: false,
            llm_cloud_enabled: false,
            daily_cap: 20,
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum TierDecision {
    Allow,
    Blocked { reason: &'static str },
}

/// Pure — no I/O. Kept for policy-matrix reasoning; cloud dispatch paths use
/// [`reserve_cloud_call`] as their authoritative admission boundary.
#[cfg(test)]
pub(crate) fn decide_cloud_tier(
    policy: &TierPolicy,
    task: CloudTaskKind,
    calls_today: u32,
    key_configured: bool,
) -> TierDecision {
    let enabled = match task {
        CloudTaskKind::Stt => policy.stt_cloud_enabled,
        CloudTaskKind::Llm => policy.llm_cloud_enabled,
    };
    if !enabled {
        return TierDecision::Blocked {
            reason: "cloud_disabled",
        };
    }
    if !key_configured {
        return TierDecision::Blocked {
            reason: "no_key_configured",
        };
    }
    if calls_today >= policy.daily_cap {
        return TierDecision::Blocked {
            reason: "cap_reached",
        };
    }
    TierDecision::Allow
}

pub(crate) fn ensure_policy_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS tier_policy (
          id INTEGER PRIMARY KEY CHECK (id = 1),
          stt_cloud_enabled INTEGER NOT NULL,
          llm_cloud_enabled INTEGER NOT NULL,
          daily_cap INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS cloud_call_counter (
          task_kind TEXT NOT NULL,
          call_date TEXT NOT NULL,
          count INTEGER NOT NULL,
          PRIMARY KEY (task_kind, call_date)
        );
        CREATE TABLE IF NOT EXISTS media_fetch_policy (
          id INTEGER PRIMARY KEY CHECK (id = 1),
          enabled INTEGER NOT NULL
        );
        "#,
    )
    .map_err(|e| e.to_string())
}

/// Whether outbound media fetching (`media_fetch`) is permitted.
///
/// A separate row from [`TierPolicy`] rather than a fourth field on it,
/// because it is a different kind of consent and collapsing them would make
/// one switch mean two things. `TierPolicy` governs sending *FUNG's own
/// material* — recorded audio, transcript text — to a paid cloud provider
/// under an API key and a daily cap. This governs pulling a stranger's URL
/// in, where nothing of the user's leaves but the address they typed. They
/// are enabled for different reasons and revoked at different times.
///
/// Absent row means absent consent: a fresh install, or one whose policy
/// database was never written, fetches nothing.
pub(crate) fn media_fetch_consent(conn: &Connection) -> Result<bool, String> {
    ensure_policy_tables(conn)?;
    conn.query_row(
        "SELECT enabled FROM media_fetch_policy WHERE id = 1",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|enabled| enabled != 0)
    .or_else(|e| {
        if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
            Ok(false)
        } else {
            Err(e.to_string())
        }
    })
}

/// Grants or revokes outbound media fetching. Reversible in both directions —
/// revoking is a plain `UPDATE`, not a tombstone, so a user who turns this
/// off is in exactly the state they were in before turning it on.
pub(crate) fn set_media_fetch_consent(conn: &Connection, enabled: bool) -> Result<(), String> {
    ensure_policy_tables(conn)?;
    conn.execute(
        "INSERT INTO media_fetch_policy (id, enabled) VALUES (1, ?1) \
         ON CONFLICT(id) DO UPDATE SET enabled = excluded.enabled",
        params![enabled as i64],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn task_kind_str(task: CloudTaskKind) -> &'static str {
    match task {
        CloudTaskKind::Stt => "stt",
        CloudTaskKind::Llm => "llm",
    }
}

/// Local calendar date (`YYYY-MM-DD`), not UTC — the cap resets when the
/// user's own day rolls over, not an arbitrary UTC midnight.
fn today_local() -> String {
    let now = std::time::SystemTime::now();
    let datetime: chrono::DateTime<chrono::Local> = now.into();
    datetime.format("%Y-%m-%d").to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CloudAdmissionError {
    Blocked { reason: &'static str },
    Persistence(String),
}

impl CloudAdmissionError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::Blocked { reason } => reason,
            Self::Persistence(_) => "policy_error",
        }
    }
}

impl fmt::Display for CloudAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blocked { reason } => f.write_str(reason),
            Self::Persistence(message) => {
                write!(f, "cloud admission persistence failed: {message}")
            }
        }
    }
}

fn read_policy_row(conn: &Connection) -> Result<TierPolicy, String> {
    let result = conn.query_row(
        "SELECT stt_cloud_enabled, llm_cloud_enabled, daily_cap FROM tier_policy WHERE id = 1",
        [],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    );
    match result {
        Ok((stt_cloud_enabled, llm_cloud_enabled, daily_cap)) => {
            let daily_cap = u32::try_from(daily_cap)
                .map_err(|_| "daily_cap is outside the supported range".to_string())?;
            Ok(TierPolicy {
                stt_cloud_enabled: stt_cloud_enabled != 0,
                llm_cloud_enabled: llm_cloud_enabled != 0,
                daily_cap,
            })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(TierPolicy::default()),
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) fn load_policy(conn: &Connection) -> Result<TierPolicy, String> {
    ensure_policy_tables(conn)?;
    read_policy_row(conn)
}

pub(crate) fn save_policy(conn: &Connection, policy: &TierPolicy) -> Result<(), String> {
    ensure_policy_tables(conn)?;
    conn.execute(
        "INSERT INTO tier_policy (id, stt_cloud_enabled, llm_cloud_enabled, daily_cap) \
         VALUES (1, ?1, ?2, ?3) \
         ON CONFLICT(id) DO UPDATE SET \
           stt_cloud_enabled = excluded.stt_cloud_enabled, \
           llm_cloud_enabled = excluded.llm_cloud_enabled, \
           daily_cap = excluded.daily_cap",
        params![
            policy.stt_cloud_enabled as i64,
            policy.llm_cloud_enabled as i64,
            policy.daily_cap
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn calls_today(conn: &Connection, task: CloudTaskKind) -> Result<u32, String> {
    ensure_policy_tables(conn)?;
    conn.query_row(
        "SELECT count FROM cloud_call_counter WHERE task_kind = ?1 AND call_date = ?2",
        params![task_kind_str(task), today_local()],
        |row| row.get::<_, i64>(0),
    )
    .or_else(|e| {
        if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
            Ok(0i64)
        } else {
            Err(e.to_string())
        }
    })
    .map(|count| count as u32)
}

fn rollback_after_failure(conn: &Connection, error: CloudAdmissionError) -> CloudAdmissionError {
    match conn.execute_batch("ROLLBACK") {
        Ok(()) => error,
        Err(rollback_error) => {
            CloudAdmissionError::Persistence(format!("{error}; rollback failed: {rollback_error}"))
        }
    }
}

/// Atomically reserves one outbound cloud slot for the current local day.
///
/// The policy and counter are read and updated inside one `BEGIN IMMEDIATE`
/// transaction. A successful commit is the only value that permits a caller
/// to contact a provider; every other result is fail-closed.
pub(crate) fn reserve_cloud_call(
    conn: &Connection,
    task: CloudTaskKind,
    key_configured: bool,
) -> Result<(), CloudAdmissionError> {
    ensure_policy_tables(conn).map_err(CloudAdmissionError::Persistence)?;
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| CloudAdmissionError::Persistence(error.to_string()))?;

    let result = (|| {
        let policy = read_policy_row(conn).map_err(CloudAdmissionError::Persistence)?;
        let enabled = match task {
            CloudTaskKind::Stt => policy.stt_cloud_enabled,
            CloudTaskKind::Llm => policy.llm_cloud_enabled,
        };
        if !enabled {
            return Err(CloudAdmissionError::Blocked {
                reason: "cloud_disabled",
            });
        }
        if !key_configured {
            return Err(CloudAdmissionError::Blocked {
                reason: "no_key_configured",
            });
        }
        if policy.daily_cap == 0 {
            return Err(CloudAdmissionError::Blocked {
                reason: "cap_reached",
            });
        }

        let changed = conn
            .execute(
                "INSERT INTO cloud_call_counter (task_kind, call_date, count) \
                 VALUES (?1, ?2, 1) \
                 ON CONFLICT(task_kind, call_date) \
                 DO UPDATE SET count = cloud_call_counter.count + 1 \
                 WHERE cloud_call_counter.count < ?3",
                params![task_kind_str(task), today_local(), policy.daily_cap],
            )
            .map_err(|error| CloudAdmissionError::Persistence(error.to_string()))?;
        if changed != 1 {
            return Err(CloudAdmissionError::Blocked {
                reason: "cap_reached",
            });
        }

        conn.execute_batch("COMMIT")
            .map_err(|error| CloudAdmissionError::Persistence(error.to_string()))
    })();

    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(rollback_after_failure(conn, error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_policy_blocks_regardless_of_cap_or_key() {
        let policy = TierPolicy {
            stt_cloud_enabled: false,
            llm_cloud_enabled: false,
            daily_cap: 100,
        };
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Stt, 0, true),
            TierDecision::Blocked {
                reason: "cloud_disabled"
            }
        );
    }

    #[test]
    fn enabled_without_key_is_blocked() {
        let policy = TierPolicy {
            stt_cloud_enabled: true,
            llm_cloud_enabled: false,
            daily_cap: 100,
        };
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Stt, 0, false),
            TierDecision::Blocked {
                reason: "no_key_configured"
            }
        );
    }

    #[test]
    fn enabled_with_key_but_cap_reached_is_blocked() {
        let policy = TierPolicy {
            stt_cloud_enabled: true,
            llm_cloud_enabled: false,
            daily_cap: 5,
        };
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Stt, 5, true),
            TierDecision::Blocked {
                reason: "cap_reached"
            }
        );
        // one under the cap is still allowed
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Stt, 4, true),
            TierDecision::Allow
        );
    }

    #[test]
    fn enabled_with_key_and_room_under_cap_is_allowed() {
        let policy = TierPolicy {
            stt_cloud_enabled: true,
            llm_cloud_enabled: true,
            daily_cap: 20,
        };
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Stt, 3, true),
            TierDecision::Allow
        );
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Llm, 3, true),
            TierDecision::Allow
        );
    }

    #[test]
    fn task_kinds_are_independent() {
        let policy = TierPolicy {
            stt_cloud_enabled: true,
            llm_cloud_enabled: false,
            daily_cap: 20,
        };
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Stt, 0, true),
            TierDecision::Allow
        );
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Llm, 0, true),
            TierDecision::Blocked {
                reason: "cloud_disabled"
            }
        );
    }

    #[test]
    fn default_policy_is_cloud_off() {
        let policy = TierPolicy::default();
        assert!(!policy.stt_cloud_enabled);
        assert!(!policy.llm_cloud_enabled);
        assert_eq!(policy.daily_cap, 20);
    }

    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_policy_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn load_policy_defaults_when_no_row_exists() {
        let conn = open_test_db();
        let policy = load_policy(&conn).unwrap();
        assert_eq!(policy, TierPolicy::default());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let conn = open_test_db();
        let policy = TierPolicy {
            stt_cloud_enabled: true,
            llm_cloud_enabled: false,
            daily_cap: 50,
        };
        save_policy(&conn, &policy).unwrap();
        assert_eq!(load_policy(&conn).unwrap(), policy);
    }

    #[test]
    fn save_twice_updates_in_place() {
        let conn = open_test_db();
        save_policy(
            &conn,
            &TierPolicy {
                stt_cloud_enabled: true,
                llm_cloud_enabled: false,
                daily_cap: 10,
            },
        )
        .unwrap();
        save_policy(
            &conn,
            &TierPolicy {
                stt_cloud_enabled: false,
                llm_cloud_enabled: true,
                daily_cap: 30,
            },
        )
        .unwrap();
        let policy = load_policy(&conn).unwrap();
        assert!(!policy.stt_cloud_enabled);
        assert!(policy.llm_cloud_enabled);
        assert_eq!(policy.daily_cap, 30);
    }

    #[test]
    fn calls_today_starts_at_zero_and_increments() {
        let conn = open_test_db();
        assert_eq!(calls_today(&conn, CloudTaskKind::Stt).unwrap(), 0);
        save_policy(
            &conn,
            &TierPolicy {
                stt_cloud_enabled: true,
                llm_cloud_enabled: false,
                daily_cap: 20,
            },
        )
        .unwrap();
        reserve_cloud_call(&conn, CloudTaskKind::Stt, true).unwrap();
        reserve_cloud_call(&conn, CloudTaskKind::Stt, true).unwrap();
        assert_eq!(calls_today(&conn, CloudTaskKind::Stt).unwrap(), 2);
    }

    #[test]
    fn calls_today_fails_closed_on_real_db_errors() {
        let conn = Connection::open_in_memory().unwrap();
        // Set up the normal table structure via ensure_policy_tables.
        ensure_policy_tables(&conn).unwrap();
        // Now corrupt the table schema: drop the count column (or replace the table with wrong schema)
        // by dropping and recreating it with a missing required column.
        conn.execute("DROP TABLE cloud_call_counter", []).unwrap();
        conn.execute(
            "CREATE TABLE cloud_call_counter (task_kind TEXT NOT NULL, call_date TEXT NOT NULL, PRIMARY KEY (task_kind, call_date))",
            [],
        )
        .unwrap();

        let result = calls_today(&conn, CloudTaskKind::Stt);
        // Must fail (Err), not silently return Ok(0).
        // The query will fail because it tries to SELECT count which doesn't exist.
        assert!(result.is_err());
    }

    #[test]
    fn calls_today_is_independent_per_task_kind() {
        let conn = open_test_db();
        save_policy(
            &conn,
            &TierPolicy {
                stt_cloud_enabled: true,
                llm_cloud_enabled: true,
                daily_cap: 2,
            },
        )
        .unwrap();
        reserve_cloud_call(&conn, CloudTaskKind::Stt, true).unwrap();
        reserve_cloud_call(&conn, CloudTaskKind::Stt, true).unwrap();
        reserve_cloud_call(&conn, CloudTaskKind::Llm, true).unwrap();
        assert_eq!(calls_today(&conn, CloudTaskKind::Stt).unwrap(), 2);
        assert_eq!(calls_today(&conn, CloudTaskKind::Llm).unwrap(), 1);
    }

    #[test]
    fn reservation_failure_is_fail_closed_and_does_not_increment() {
        let conn = open_test_db();
        save_policy(
            &conn,
            &TierPolicy {
                stt_cloud_enabled: true,
                llm_cloud_enabled: false,
                daily_cap: 2,
            },
        )
        .unwrap();
        conn.execute_batch(
            "CREATE TRIGGER block_counter_insert BEFORE INSERT ON cloud_call_counter \
             BEGIN SELECT RAISE(ABORT, 'simulated counter write failure'); END;",
        )
        .unwrap();

        let error = reserve_cloud_call(&conn, CloudTaskKind::Stt, true).unwrap_err();
        assert_eq!(error.code(), "policy_error");
        assert_eq!(calls_today(&conn, CloudTaskKind::Stt).unwrap(), 0);
    }

    #[test]
    fn reservations_keep_stt_and_llm_counters_independent() {
        let conn = open_test_db();
        save_policy(
            &conn,
            &TierPolicy {
                stt_cloud_enabled: true,
                llm_cloud_enabled: true,
                daily_cap: 2,
            },
        )
        .unwrap();

        reserve_cloud_call(&conn, CloudTaskKind::Stt, true).unwrap();
        reserve_cloud_call(&conn, CloudTaskKind::Stt, true).unwrap();
        assert_eq!(
            reserve_cloud_call(&conn, CloudTaskKind::Stt, true),
            Err(CloudAdmissionError::Blocked {
                reason: "cap_reached"
            })
        );

        reserve_cloud_call(&conn, CloudTaskKind::Llm, true).unwrap();
        assert_eq!(calls_today(&conn, CloudTaskKind::Llm).unwrap(), 1);
    }

    #[test]
    fn concurrent_reservations_never_exceed_the_shared_cap_per_task_kind() {
        use std::sync::{Arc, Barrier};

        let db_path = std::env::temp_dir().join(format!("fung-policy-{}.db", uuid::Uuid::new_v4()));
        let setup = Connection::open(&db_path).unwrap();
        setup.pragma_update(None, "journal_mode", "WAL").unwrap();
        setup.pragma_update(None, "busy_timeout", 5000).unwrap();
        ensure_policy_tables(&setup).unwrap();
        save_policy(
            &setup,
            &TierPolicy {
                stt_cloud_enabled: true,
                llm_cloud_enabled: false,
                daily_cap: 3,
            },
        )
        .unwrap();
        drop(setup);

        let barrier = Arc::new(Barrier::new(8));
        let mut workers = Vec::new();
        for _ in 0..8 {
            let path = db_path.clone();
            let barrier = Arc::clone(&barrier);
            workers.push(std::thread::spawn(move || {
                let conn = Connection::open(path).unwrap();
                conn.pragma_update(None, "journal_mode", "WAL").unwrap();
                conn.pragma_update(None, "busy_timeout", 5000).unwrap();
                barrier.wait();
                reserve_cloud_call(&conn, CloudTaskKind::Stt, true)
            }));
        }

        let results: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        let allowed = results.iter().filter(|result| result.is_ok()).count();
        let blocked = results
            .iter()
            .filter(|result| {
                matches!(
                    result,
                    Err(CloudAdmissionError::Blocked {
                        reason: "cap_reached"
                    })
                )
            })
            .count();
        assert_eq!(allowed, 3, "exactly the cap may commit: {results:?}");
        assert_eq!(
            blocked, 5,
            "all remaining reservations must block: {results:?}"
        );

        let check = Connection::open(&db_path).unwrap();
        assert_eq!(calls_today(&check, CloudTaskKind::Stt).unwrap(), 3);
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn media_fetch_consent_defaults_to_withheld() {
        // A fresh install must not be able to reach the network because
        // nobody has said anything yet. Absent row, absent consent.
        let conn = open_test_db();
        assert!(!media_fetch_consent(&conn).unwrap());
    }

    #[test]
    fn media_fetch_consent_is_reversible() {
        let conn = open_test_db();
        set_media_fetch_consent(&conn, true).unwrap();
        assert!(media_fetch_consent(&conn).unwrap());
        set_media_fetch_consent(&conn, false).unwrap();
        assert!(!media_fetch_consent(&conn).unwrap());
    }

    #[test]
    fn media_fetch_consent_is_independent_of_the_cloud_tier() {
        // Enabling cloud STT is consent to send *this machine's audio out*.
        // It must not also authorise pulling arbitrary URLs in: they are
        // different decisions and the UI presents them as such.
        let conn = open_test_db();
        save_policy(
            &conn,
            &TierPolicy {
                stt_cloud_enabled: true,
                llm_cloud_enabled: true,
                daily_cap: 100,
            },
        )
        .unwrap();
        assert!(!media_fetch_consent(&conn).unwrap());
    }

    #[test]
    fn media_fetch_consent_fails_closed_on_real_db_errors() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_policy_tables(&conn).unwrap();
        conn.execute("DROP TABLE media_fetch_policy", []).unwrap();
        conn.execute(
            "CREATE TABLE media_fetch_policy (id INTEGER PRIMARY KEY)",
            [],
        )
        .unwrap();

        // A broken schema must not read as "consent granted", and must not
        // read as a silent `false` either -- the caller has to know.
        assert!(media_fetch_consent(&conn).is_err());
    }
}
