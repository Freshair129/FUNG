//! Tier-3 (cloud) fallback policy: a pure decision function plus its two
//! bits of local SQLite-backed state (the policy row, the daily call
//! counter). No secrets live here — cloud API keys stay in cloud_config.rs's
//! keyring entries; this module only decides whether cloud is *allowed*.

use crate::cloud_config::CloudTaskKind;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TierPolicy {
    pub(crate) stt_cloud_enabled: bool,
    pub(crate) llm_cloud_enabled: bool,
    pub(crate) daily_cap: u32,
}

impl Default for TierPolicy {
    fn default() -> Self {
        Self { stt_cloud_enabled: false, llm_cloud_enabled: false, daily_cap: 20 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum TierDecision {
    Allow,
    Blocked { reason: &'static str },
}

/// Pure — no I/O. Callers (fungwire_server.rs for STT, graph_build.rs for
/// LLM) read `calls_today`/`key_configured` themselves before calling this.
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
        return TierDecision::Blocked { reason: "cloud_disabled" };
    }
    if !key_configured {
        return TierDecision::Blocked { reason: "no_key_configured" };
    }
    if calls_today >= policy.daily_cap {
        return TierDecision::Blocked { reason: "cap_reached" };
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
        "#,
    )
    .map_err(|e| e.to_string())
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

pub(crate) fn load_policy(conn: &Connection) -> Result<TierPolicy, String> {
    ensure_policy_tables(conn)?;
    conn.query_row(
        "SELECT stt_cloud_enabled, llm_cloud_enabled, daily_cap FROM tier_policy WHERE id = 1",
        [],
        |row| {
            Ok(TierPolicy {
                stt_cloud_enabled: row.get::<_, i64>(0)? != 0,
                llm_cloud_enabled: row.get::<_, i64>(1)? != 0,
                daily_cap: row.get::<_, i64>(2)? as u32,
            })
        },
    )
    .or_else(|e| {
        if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
            Ok(TierPolicy::default())
        } else {
            Err(e.to_string())
        }
    })
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
        params![policy.stt_cloud_enabled as i64, policy.llm_cloud_enabled as i64, policy.daily_cap],
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

pub(crate) fn increment_calls_today(conn: &Connection, task: CloudTaskKind) -> Result<(), String> {
    ensure_policy_tables(conn)?;
    conn.execute(
        "INSERT INTO cloud_call_counter (task_kind, call_date, count) VALUES (?1, ?2, 1) \
         ON CONFLICT(task_kind, call_date) DO UPDATE SET count = count + 1",
        params![task_kind_str(task), today_local()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_policy_blocks_regardless_of_cap_or_key() {
        let policy = TierPolicy { stt_cloud_enabled: false, llm_cloud_enabled: false, daily_cap: 100 };
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Stt, 0, true),
            TierDecision::Blocked { reason: "cloud_disabled" }
        );
    }

    #[test]
    fn enabled_without_key_is_blocked() {
        let policy = TierPolicy { stt_cloud_enabled: true, llm_cloud_enabled: false, daily_cap: 100 };
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Stt, 0, false),
            TierDecision::Blocked { reason: "no_key_configured" }
        );
    }

    #[test]
    fn enabled_with_key_but_cap_reached_is_blocked() {
        let policy = TierPolicy { stt_cloud_enabled: true, llm_cloud_enabled: false, daily_cap: 5 };
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Stt, 5, true),
            TierDecision::Blocked { reason: "cap_reached" }
        );
        // one under the cap is still allowed
        assert_eq!(decide_cloud_tier(&policy, CloudTaskKind::Stt, 4, true), TierDecision::Allow);
    }

    #[test]
    fn enabled_with_key_and_room_under_cap_is_allowed() {
        let policy = TierPolicy { stt_cloud_enabled: true, llm_cloud_enabled: true, daily_cap: 20 };
        assert_eq!(decide_cloud_tier(&policy, CloudTaskKind::Stt, 3, true), TierDecision::Allow);
        assert_eq!(decide_cloud_tier(&policy, CloudTaskKind::Llm, 3, true), TierDecision::Allow);
    }

    #[test]
    fn task_kinds_are_independent() {
        let policy = TierPolicy { stt_cloud_enabled: true, llm_cloud_enabled: false, daily_cap: 20 };
        assert_eq!(decide_cloud_tier(&policy, CloudTaskKind::Stt, 0, true), TierDecision::Allow);
        assert_eq!(
            decide_cloud_tier(&policy, CloudTaskKind::Llm, 0, true),
            TierDecision::Blocked { reason: "cloud_disabled" }
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
        let policy = TierPolicy { stt_cloud_enabled: true, llm_cloud_enabled: false, daily_cap: 50 };
        save_policy(&conn, &policy).unwrap();
        assert_eq!(load_policy(&conn).unwrap(), policy);
    }

    #[test]
    fn save_twice_updates_in_place() {
        let conn = open_test_db();
        save_policy(&conn, &TierPolicy { stt_cloud_enabled: true, llm_cloud_enabled: false, daily_cap: 10 }).unwrap();
        save_policy(&conn, &TierPolicy { stt_cloud_enabled: false, llm_cloud_enabled: true, daily_cap: 30 }).unwrap();
        let policy = load_policy(&conn).unwrap();
        assert!(!policy.stt_cloud_enabled);
        assert!(policy.llm_cloud_enabled);
        assert_eq!(policy.daily_cap, 30);
    }

    #[test]
    fn calls_today_starts_at_zero_and_increments() {
        let conn = open_test_db();
        assert_eq!(calls_today(&conn, CloudTaskKind::Stt).unwrap(), 0);
        increment_calls_today(&conn, CloudTaskKind::Stt).unwrap();
        increment_calls_today(&conn, CloudTaskKind::Stt).unwrap();
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
        increment_calls_today(&conn, CloudTaskKind::Stt).unwrap();
        increment_calls_today(&conn, CloudTaskKind::Stt).unwrap();
        increment_calls_today(&conn, CloudTaskKind::Llm).unwrap();
        assert_eq!(calls_today(&conn, CloudTaskKind::Stt).unwrap(), 2);
        assert_eq!(calls_today(&conn, CloudTaskKind::Llm).unwrap(), 1);
    }
}
