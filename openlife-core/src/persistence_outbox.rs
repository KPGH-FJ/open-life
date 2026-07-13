//! Metadata-only transactional outbox primitives for canonical SQLite owners.
//!
//! This module deliberately does not provide a cross-database transaction. A
//! canonical SQLite owner records its mutation, tombstone (when applicable),
//! outbox event, and per-projection delivery rows in the *same* local
//! transaction. Projection consumers then reconcile other stores
//! idempotently. File-backed canonical owners need a separate durable journal
//! and digest reconciliation; they must not use this API to claim atomicity.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

const MAX_ID_LEN: usize = 256;
const MAX_PROJECTION_TARGETS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalProjectionHeadAdvanced {
    pub expected_event_id: String,
    pub expected_revision: u64,
    pub actual_event_id: String,
    pub actual_revision: u64,
}

impl std::fmt::Display for CanonicalProjectionHeadAdvanced {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "canonical projection head advanced from {}@{} to {}@{}",
            self.expected_event_id,
            self.expected_revision,
            self.actual_event_id,
            self.actual_revision
        )
    }
}

impl std::error::Error for CanonicalProjectionHeadAdvanced {}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionDeliveryState {
    #[default]
    Pending,
    Degraded,
    Applied,
    Superseded,
    Compensated,
}

impl ProjectionDeliveryState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Degraded => "degraded",
            Self::Applied => "applied",
            Self::Superseded => "superseded",
            Self::Compensated => "compensated",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "degraded" => Ok(Self::Degraded),
            "applied" => Ok(Self::Applied),
            "superseded" => Ok(Self::Superseded),
            "compensated" => Ok(Self::Compensated),
            other => anyhow::bail!("unsupported projection delivery state: {other}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalMutationReceipt {
    pub event_id: String,
    pub aggregate_kind: String,
    pub aggregate_id: String,
    pub mutation_kind: String,
    pub aggregate_revision: u64,
    pub payload_digest: String,
    pub tombstone_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionDelivery {
    pub event_id: String,
    pub aggregate_kind: String,
    pub aggregate_id: String,
    pub mutation_kind: String,
    pub aggregate_revision: u64,
    pub payload_digest: String,
    pub tombstone_id: Option<String>,
    pub projection_target: String,
    pub state: ProjectionDeliveryState,
    pub attempt_count: u32,
    pub last_error_digest: Option<String>,
    pub terminal_disposition: Option<String>,
    pub superseded_by_event_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionSummary {
    pub event_id: String,
    pub pending: u32,
    pub degraded: u32,
    pub applied: u32,
    pub superseded: u32,
    pub compensated: u32,
}

impl ProjectionSummary {
    pub fn state(&self) -> ProjectionDeliveryState {
        if self.degraded > 0 {
            ProjectionDeliveryState::Degraded
        } else if self.pending > 0 {
            ProjectionDeliveryState::Pending
        } else if self.compensated > 0 {
            ProjectionDeliveryState::Compensated
        } else if self.superseded > 0 {
            ProjectionDeliveryState::Superseded
        } else {
            ProjectionDeliveryState::Applied
        }
    }
}

pub fn metadata_digest(value: &str) -> String {
    let digest = digest(&SHA256, value.as_bytes());
    let mut encoded = String::with_capacity(64);
    for byte in digest.as_ref() {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    format!("sha256:{encoded}")
}

pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS canonical_tombstones (
            tombstone_id TEXT PRIMARY KEY,
            aggregate_kind TEXT NOT NULL,
            aggregate_id TEXT NOT NULL,
            reason_digest TEXT,
            created_at TEXT NOT NULL,
            superseded_at TEXT,
            superseded_by_event_id TEXT,
            UNIQUE(aggregate_kind, aggregate_id, superseded_at)
         );
         CREATE UNIQUE INDEX IF NOT EXISTS idx_canonical_tombstones_active
         ON canonical_tombstones(aggregate_kind, aggregate_id)
         WHERE superseded_at IS NULL;
         CREATE TABLE IF NOT EXISTS canonical_outbox_events (
            event_id TEXT PRIMARY KEY,
            aggregate_kind TEXT NOT NULL,
            aggregate_id TEXT NOT NULL,
            mutation_kind TEXT NOT NULL,
            aggregate_revision INTEGER NOT NULL,
            payload_digest TEXT NOT NULL,
            tombstone_id TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY(tombstone_id) REFERENCES canonical_tombstones(tombstone_id)
         );
         CREATE INDEX IF NOT EXISTS idx_canonical_outbox_aggregate
         ON canonical_outbox_events(aggregate_kind, aggregate_id, created_at);
         CREATE TABLE IF NOT EXISTS canonical_outbox_deliveries (
            event_id TEXT NOT NULL,
            projection_target TEXT NOT NULL,
            state TEXT NOT NULL CHECK(state IN ('pending', 'degraded', 'applied')),
            attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count >= 0),
            last_error_digest TEXT,
            terminal_disposition TEXT CHECK(
                terminal_disposition IS NULL OR
                terminal_disposition IN ('superseded', 'compensated')
            ),
            superseded_by_event_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY(event_id, projection_target),
            FOREIGN KEY(event_id) REFERENCES canonical_outbox_events(event_id) ON DELETE CASCADE
         ) WITHOUT ROWID;
         CREATE INDEX IF NOT EXISTS idx_canonical_outbox_delivery_queue
         ON canonical_outbox_deliveries(state, updated_at, event_id);",
    )?;
    crate::sqlite_migration::ensure_column(
        conn,
        "canonical_outbox_events",
        "aggregate_revision",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    crate::sqlite_migration::ensure_column(
        conn,
        "canonical_outbox_deliveries",
        "terminal_disposition",
        "TEXT",
    )?;
    crate::sqlite_migration::ensure_column(
        conn,
        "canonical_outbox_deliveries",
        "superseded_by_event_id",
        "TEXT",
    )?;
    conn.execute_batch(
        "UPDATE canonical_outbox_events AS current
         SET aggregate_revision = (
             SELECT COUNT(*) FROM canonical_outbox_events AS prior
             WHERE prior.aggregate_kind = current.aggregate_kind
               AND prior.aggregate_id = current.aggregate_id
               AND (
                   prior.created_at < current.created_at OR
                   (prior.created_at = current.created_at AND prior.event_id <= current.event_id)
               )
         )
         WHERE aggregate_revision = 0;
         CREATE UNIQUE INDEX IF NOT EXISTS idx_canonical_outbox_aggregate_revision
         ON canonical_outbox_events(aggregate_kind, aggregate_id, aggregate_revision);",
    )?;
    Ok(())
}

pub fn enqueue_mutation(
    tx: &Transaction<'_>,
    aggregate_kind: &str,
    aggregate_id: &str,
    mutation_kind: &str,
    payload_digest: &str,
    projection_targets: &[&str],
) -> Result<CanonicalMutationReceipt> {
    enqueue_event(
        tx,
        aggregate_kind,
        aggregate_id,
        mutation_kind,
        payload_digest,
        None,
        projection_targets,
    )
}

pub fn enqueue_tombstone(
    tx: &Transaction<'_>,
    aggregate_kind: &str,
    aggregate_id: &str,
    reason: Option<&str>,
    projection_targets: &[&str],
) -> Result<CanonicalMutationReceipt> {
    validate_identifier("aggregate_kind", aggregate_kind)?;
    validate_identifier("aggregate_id", aggregate_id)?;
    validate_targets(projection_targets)?;

    if let Some(existing) = active_tombstone_event(tx, aggregate_kind, aggregate_id)? {
        ensure_delivery_targets(tx, &existing, projection_targets)?;
        return Ok(existing);
    }

    let created_at = Utc::now();
    let tombstone_id = format!("tombstone:{}", Uuid::new_v4());
    let event_id = format!("outbox:{}", Uuid::new_v4());
    let reason_digest = reason.map(metadata_digest);
    let payload_digest = metadata_digest(&format!(
        "{aggregate_kind}:{aggregate_id}:deleted:{tombstone_id}"
    ));
    tx.execute(
        "INSERT INTO canonical_tombstones (
            tombstone_id, aggregate_kind, aggregate_id, reason_digest, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            tombstone_id,
            aggregate_kind,
            aggregate_id,
            reason_digest,
            created_at.to_rfc3339()
        ],
    )?;
    let receipt = insert_event(
        tx,
        event_id,
        aggregate_kind,
        aggregate_id,
        "deleted",
        payload_digest,
        Some(tombstone_id),
        created_at,
        projection_targets,
    )?;
    Ok(receipt)
}

pub fn supersede_active_tombstone(
    tx: &Transaction<'_>,
    aggregate_kind: &str,
    aggregate_id: &str,
    payload_digest: &str,
    projection_targets: &[&str],
) -> Result<CanonicalMutationReceipt> {
    validate_identifier("aggregate_kind", aggregate_kind)?;
    validate_identifier("aggregate_id", aggregate_id)?;
    let receipt = enqueue_event(
        tx,
        aggregate_kind,
        aggregate_id,
        "restored",
        payload_digest,
        None,
        projection_targets,
    )?;
    tx.execute(
        "UPDATE canonical_tombstones
         SET superseded_at = ?3, superseded_by_event_id = ?4
         WHERE aggregate_kind = ?1 AND aggregate_id = ?2 AND superseded_at IS NULL",
        params![
            aggregate_kind,
            aggregate_id,
            receipt.created_at.to_rfc3339(),
            receipt.event_id
        ],
    )?;
    Ok(receipt)
}

fn enqueue_event(
    tx: &Transaction<'_>,
    aggregate_kind: &str,
    aggregate_id: &str,
    mutation_kind: &str,
    payload_digest: &str,
    tombstone_id: Option<String>,
    projection_targets: &[&str],
) -> Result<CanonicalMutationReceipt> {
    validate_identifier("aggregate_kind", aggregate_kind)?;
    validate_identifier("aggregate_id", aggregate_id)?;
    validate_identifier("mutation_kind", mutation_kind)?;
    validate_digest(payload_digest)?;
    validate_targets(projection_targets)?;
    insert_event(
        tx,
        format!("outbox:{}", Uuid::new_v4()),
        aggregate_kind,
        aggregate_id,
        mutation_kind,
        payload_digest.to_string(),
        tombstone_id,
        Utc::now(),
        projection_targets,
    )
}

#[allow(clippy::too_many_arguments)]
fn insert_event(
    tx: &Transaction<'_>,
    event_id: String,
    aggregate_kind: &str,
    aggregate_id: &str,
    mutation_kind: &str,
    payload_digest: String,
    tombstone_id: Option<String>,
    created_at: DateTime<Utc>,
    projection_targets: &[&str],
) -> Result<CanonicalMutationReceipt> {
    let aggregate_revision_raw: i64 = tx.query_row(
        "SELECT COALESCE(MAX(aggregate_revision), 0) + 1
         FROM canonical_outbox_events
         WHERE aggregate_kind = ?1 AND aggregate_id = ?2",
        params![aggregate_kind, aggregate_id],
        |row| row.get(0),
    )?;
    let aggregate_revision =
        u64::try_from(aggregate_revision_raw).context("canonical aggregate revision is invalid")?;
    tx.execute(
        "INSERT INTO canonical_outbox_events (
            event_id, aggregate_kind, aggregate_id, mutation_kind,
            aggregate_revision, payload_digest, tombstone_id, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            event_id,
            aggregate_kind,
            aggregate_id,
            mutation_kind,
            aggregate_revision_raw,
            payload_digest,
            tombstone_id,
            created_at.to_rfc3339()
        ],
    )?;
    let receipt = CanonicalMutationReceipt {
        event_id,
        aggregate_kind: aggregate_kind.to_string(),
        aggregate_id: aggregate_id.to_string(),
        mutation_kind: mutation_kind.to_string(),
        aggregate_revision,
        payload_digest,
        tombstone_id,
        created_at,
    };
    tx.execute(
        "UPDATE canonical_outbox_deliveries
         SET terminal_disposition = 'superseded',
             superseded_by_event_id = ?3,
             updated_at = ?4
         WHERE terminal_disposition IS NULL
           AND state != 'applied'
           AND event_id IN (
               SELECT event_id FROM canonical_outbox_events
               WHERE aggregate_kind = ?1 AND aggregate_id = ?2
                 AND aggregate_revision < ?5
           )",
        params![
            aggregate_kind,
            aggregate_id,
            receipt.event_id,
            created_at.to_rfc3339(),
            aggregate_revision_raw,
        ],
    )?;
    ensure_delivery_targets(tx, &receipt, projection_targets)?;
    Ok(receipt)
}

fn ensure_delivery_targets(
    tx: &Transaction<'_>,
    receipt: &CanonicalMutationReceipt,
    projection_targets: &[&str],
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    for target in projection_targets {
        tx.execute(
            "INSERT INTO canonical_outbox_deliveries (
                event_id, projection_target, state, attempt_count,
                last_error_digest, created_at, updated_at
             ) VALUES (?1, ?2, 'pending', 0, NULL, ?3, ?3)
             ON CONFLICT(event_id, projection_target) DO NOTHING",
            params![receipt.event_id, target, now],
        )?;
    }
    Ok(())
}

fn active_tombstone_event(
    conn: &Connection,
    aggregate_kind: &str,
    aggregate_id: &str,
) -> Result<Option<CanonicalMutationReceipt>> {
    conn.query_row(
        "SELECT events.event_id, events.aggregate_kind, events.aggregate_id,
                events.mutation_kind, events.aggregate_revision,
                events.payload_digest, events.tombstone_id, events.created_at
         FROM canonical_tombstones tombstones
         JOIN canonical_outbox_events events
           ON events.tombstone_id = tombstones.tombstone_id
         WHERE tombstones.aggregate_kind = ?1
           AND tombstones.aggregate_id = ?2
           AND tombstones.superseded_at IS NULL
         ORDER BY events.created_at ASC
         LIMIT 1",
        params![aggregate_kind, aggregate_id],
        row_to_receipt,
    )
    .optional()
    .map_err(Into::into)
}

/// Read the canonical deletion fence inside the caller's current SQLite
/// transaction. Domain writers use this immediately before mutation so a
/// deleted aggregate cannot be recreated by a late request. Recovery must use
/// `supersede_active_tombstone` and emit an explicit `restored` outbox event;
/// deleting or ignoring the fence is not a restore contract.
pub fn has_active_tombstone(
    conn: &Connection,
    aggregate_kind: &str,
    aggregate_id: &str,
) -> Result<bool> {
    validate_identifier("aggregate_kind", aggregate_kind)?;
    validate_identifier("aggregate_id", aggregate_id)?;
    Ok(conn
        .query_row(
            "SELECT 1 FROM canonical_tombstones
             WHERE aggregate_kind = ?1 AND aggregate_id = ?2
               AND superseded_at IS NULL
             LIMIT 1",
            params![aggregate_kind, aggregate_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// Return the canonical tombstone ids that a restore event superseded in the
/// same owner transaction. Projection stores persist these ids as inactive
/// fences so an already-loaded late delete delivery cannot reactivate stale
/// visibility state after restore.
pub fn superseded_tombstone_ids_for_event(
    conn: &Connection,
    superseding_event_id: &str,
) -> Result<Vec<String>> {
    validate_identifier("superseding_event_id", superseding_event_id)?;
    let mut statement = conn.prepare(
        "SELECT tombstone_id FROM canonical_tombstones
         WHERE superseded_by_event_id = ?1
         ORDER BY tombstone_id ASC",
    )?;
    let tombstone_ids = statement
        .query_map([superseding_event_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(anyhow::Error::from)?;
    Ok(tombstone_ids)
}

pub fn latest_mutation_for_aggregate(
    conn: &Connection,
    aggregate_kind: &str,
    aggregate_id: &str,
) -> Result<Option<CanonicalMutationReceipt>> {
    validate_identifier("aggregate_kind", aggregate_kind)?;
    validate_identifier("aggregate_id", aggregate_id)?;
    conn.query_row(
        "SELECT event_id, aggregate_kind, aggregate_id, mutation_kind,
                aggregate_revision, payload_digest, tombstone_id, created_at
         FROM canonical_outbox_events
         WHERE aggregate_kind = ?1 AND aggregate_id = ?2
         ORDER BY aggregate_revision DESC LIMIT 1",
        params![aggregate_kind, aggregate_id],
        row_to_receipt,
    )
    .optional()
    .map_err(Into::into)
}

/// Reload one exact canonical mutation receipt by its immutable event id.
///
/// Idempotent command journals use this lookup to return the receipt from the
/// original transaction. They must not approximate a replay with the latest
/// aggregate event because a later mutation may already exist.
pub fn mutation_by_event_id(
    conn: &Connection,
    event_id: &str,
) -> Result<Option<CanonicalMutationReceipt>> {
    validate_identifier("event_id", event_id)?;
    conn.query_row(
        "SELECT event_id, aggregate_kind, aggregate_id, mutation_kind,
                aggregate_revision, payload_digest, tombstone_id, created_at
         FROM canonical_outbox_events
         WHERE event_id = ?1",
        [event_id],
        row_to_receipt,
    )
    .optional()
    .map_err(Into::into)
}

pub fn tombstone_ids_for_aggregate(
    conn: &Connection,
    aggregate_kind: &str,
    aggregate_id: &str,
) -> Result<Vec<String>> {
    validate_identifier("aggregate_kind", aggregate_kind)?;
    validate_identifier("aggregate_id", aggregate_id)?;
    let mut statement = conn.prepare(
        "SELECT tombstone_id FROM canonical_tombstones
         WHERE aggregate_kind = ?1 AND aggregate_id = ?2
         ORDER BY created_at ASC, tombstone_id ASC",
    )?;
    let ids = statement
        .query_map(params![aggregate_kind, aggregate_id], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(ids)
}

pub fn list_replayable_deliveries(
    conn: &Connection,
    limit: usize,
) -> Result<Vec<ProjectionDelivery>> {
    let bounded_limit = limit.clamp(1, 500);
    let mut statement = conn.prepare(
        "SELECT events.event_id, events.aggregate_kind, events.aggregate_id,
                events.mutation_kind, events.aggregate_revision,
                events.payload_digest, events.tombstone_id,
                deliveries.projection_target, deliveries.state,
                deliveries.attempt_count, deliveries.last_error_digest,
                deliveries.terminal_disposition, deliveries.superseded_by_event_id,
                deliveries.created_at, deliveries.updated_at
         FROM canonical_outbox_deliveries deliveries
         JOIN canonical_outbox_events events ON events.event_id = deliveries.event_id
         WHERE deliveries.state IN ('pending', 'degraded')
           AND deliveries.terminal_disposition IS NULL
           AND NOT EXISTS (
               SELECT 1 FROM canonical_tombstones tombstones
               WHERE tombstones.tombstone_id = events.tombstone_id
                 AND tombstones.superseded_by_event_id IS NOT NULL
           )
         ORDER BY CASE
                    WHEN events.tombstone_id IS NOT NULL
                      OR events.mutation_kind = 'restored' THEN 0
                    ELSE 1
                  END ASC,
                  deliveries.updated_at ASC, deliveries.event_id ASC,
                  deliveries.projection_target ASC
         LIMIT ?1",
    )?;
    let rows = statement.query_map([bounded_limit as i64], row_to_delivery)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

/// Load only the still-replayable deliveries for one canonical event. This is
/// the foreground isolation primitive: callers can reconcile the mutation
/// they just committed without draining unrelated owners or historical
/// embedding backlogs.
pub fn list_replayable_deliveries_for_event(
    conn: &Connection,
    event_id: &str,
) -> Result<Vec<ProjectionDelivery>> {
    validate_identifier("event_id", event_id)?;
    let mut statement = conn.prepare(
        "SELECT events.event_id, events.aggregate_kind, events.aggregate_id,
                events.mutation_kind, events.aggregate_revision,
                events.payload_digest, events.tombstone_id,
                deliveries.projection_target, deliveries.state,
                deliveries.attempt_count, deliveries.last_error_digest,
                deliveries.terminal_disposition, deliveries.superseded_by_event_id,
                deliveries.created_at, deliveries.updated_at
         FROM canonical_outbox_deliveries deliveries
         JOIN canonical_outbox_events events ON events.event_id = deliveries.event_id
         WHERE deliveries.event_id = ?1
           AND deliveries.state IN ('pending', 'degraded')
           AND deliveries.terminal_disposition IS NULL
           AND NOT EXISTS (
               SELECT 1 FROM canonical_tombstones tombstones
               WHERE tombstones.tombstone_id = events.tombstone_id
                 AND tombstones.superseded_by_event_id IS NOT NULL
           )
         ORDER BY CASE
                    WHEN events.tombstone_id IS NOT NULL
                      OR events.mutation_kind = 'restored' THEN 0
                    ELSE 1
                  END ASC,
                  deliveries.projection_target ASC",
    )?;
    let rows = statement.query_map([event_id], row_to_delivery)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub fn mark_delivery_applied(
    conn: &Connection,
    event_id: &str,
    projection_target: &str,
) -> Result<()> {
    validate_identifier("event_id", event_id)?;
    validate_identifier("projection_target", projection_target)?;
    let changed = conn.execute(
        "UPDATE canonical_outbox_deliveries
         SET state = 'applied', attempt_count = attempt_count + 1,
             last_error_digest = NULL, updated_at = ?3
         WHERE event_id = ?1 AND projection_target = ?2 AND state != 'applied'
           AND terminal_disposition IS NULL",
        params![event_id, projection_target, Utc::now().to_rfc3339()],
    )?;
    if changed == 0 {
        let state = delivery_state(conn, event_id, projection_target)?;
        if state != Some(ProjectionDeliveryState::Applied) {
            anyhow::bail!("canonical outbox delivery missing: {event_id}:{projection_target}");
        }
    }
    Ok(())
}

/// Finalize an AgentRun-style state projection only while the event is still
/// the durable aggregate head. A process-local lock closes ordinary in-process
/// TOCTOU; this transaction is the cross-process/restart truth check.
pub fn mark_delivery_applied_if_canonical_head(
    conn: &mut Connection,
    event_id: &str,
    aggregate_revision: u64,
    projection_target: &str,
) -> Result<()> {
    validate_identifier("event_id", event_id)?;
    validate_identifier("projection_target", projection_target)?;
    if aggregate_revision == 0 {
        anyhow::bail!("canonical delivery finalization requires a non-zero revision");
    }
    let expected_revision = i64::try_from(aggregate_revision)
        .context("canonical delivery finalization revision overflow")?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let (aggregate_kind, aggregate_id, persisted_revision): (String, String, i64) = tx
        .query_row(
            "SELECT aggregate_kind, aggregate_id, aggregate_revision
             FROM canonical_outbox_events WHERE event_id = ?1",
            [event_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .with_context(|| format!("canonical outbox event missing: {event_id}"))?;
    if persisted_revision != expected_revision {
        anyhow::bail!(
            "canonical delivery finalization revision mismatch: persisted={persisted_revision}, expected={expected_revision}"
        );
    }
    let (actual_event_id, actual_revision_raw): (String, i64) = tx.query_row(
        "SELECT event_id, aggregate_revision
         FROM canonical_outbox_events
         WHERE aggregate_kind = ?1 AND aggregate_id = ?2
         ORDER BY aggregate_revision DESC LIMIT 1",
        params![aggregate_kind, aggregate_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let actual_revision = u64::try_from(actual_revision_raw)
        .context("canonical delivery finalization actual revision is invalid")?;
    if actual_event_id != event_id || actual_revision_raw != expected_revision {
        return Err(CanonicalProjectionHeadAdvanced {
            expected_event_id: event_id.to_string(),
            expected_revision: aggregate_revision,
            actual_event_id,
            actual_revision,
        }
        .into());
    }
    let changed = tx.execute(
        "UPDATE canonical_outbox_deliveries
         SET state = 'applied', attempt_count = attempt_count + 1,
             last_error_digest = NULL, updated_at = ?3
         WHERE event_id = ?1 AND projection_target = ?2 AND state != 'applied'
           AND terminal_disposition IS NULL",
        params![event_id, projection_target, Utc::now().to_rfc3339()],
    )?;
    if changed == 0 {
        let applied_and_current = tx
            .query_row(
                "SELECT 1 FROM canonical_outbox_deliveries
                 WHERE event_id = ?1 AND projection_target = ?2
                   AND state = 'applied' AND terminal_disposition IS NULL",
                params![event_id, projection_target],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !applied_and_current {
            anyhow::bail!(
                "canonical head delivery missing for finalization: {event_id}:{projection_target}"
            );
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn mark_delivery_degraded(
    conn: &Connection,
    event_id: &str,
    projection_target: &str,
    error: &str,
) -> Result<()> {
    validate_identifier("event_id", event_id)?;
    validate_identifier("projection_target", projection_target)?;
    let changed = conn.execute(
        "UPDATE canonical_outbox_deliveries
         SET state = 'degraded', attempt_count = attempt_count + 1,
             last_error_digest = ?3, updated_at = ?4
         WHERE event_id = ?1 AND projection_target = ?2 AND state != 'applied'
           AND terminal_disposition IS NULL",
        params![
            event_id,
            projection_target,
            metadata_digest(error),
            Utc::now().to_rfc3339()
        ],
    )?;
    if changed == 0 && delivery_state(conn, event_id, projection_target)?.is_none() {
        anyhow::bail!("canonical outbox delivery missing: {event_id}:{projection_target}");
    }
    Ok(())
}

/// Atomically records that a stale delivery was used to bring its projection
/// target to the current canonical head, and marks the head delivery applied.
/// This is terminal truth, not a retryable failure and not a false claim that
/// the stale mutation itself was applied.
pub fn mark_delivery_compensated_to_head(
    conn: &mut Connection,
    stale_event_id: &str,
    head_event_id: &str,
    head_revision: u64,
    projection_target: &str,
) -> Result<()> {
    validate_identifier("stale_event_id", stale_event_id)?;
    validate_identifier("head_event_id", head_event_id)?;
    validate_identifier("projection_target", projection_target)?;
    if stale_event_id == head_event_id {
        anyhow::bail!("compensation requires distinct stale and head events");
    }
    if head_revision == 0 {
        anyhow::bail!("compensation requires a non-zero canonical head revision");
    }
    let expected_head_revision =
        i64::try_from(head_revision).context("canonical compensation head revision overflow")?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let (stale_kind, stale_aggregate_id, stale_revision): (String, String, i64) = tx
        .query_row(
            "SELECT aggregate_kind, aggregate_id, aggregate_revision
             FROM canonical_outbox_events WHERE event_id = ?1",
            [stale_event_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .with_context(|| format!("stale canonical outbox event missing: {stale_event_id}"))?;
    let (head_kind, head_aggregate_id, persisted_head_revision): (String, String, i64) = tx
        .query_row(
            "SELECT aggregate_kind, aggregate_id, aggregate_revision
             FROM canonical_outbox_events WHERE event_id = ?1",
            [head_event_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .with_context(|| format!("canonical compensation head missing: {head_event_id}"))?;
    if stale_kind != head_kind || stale_aggregate_id != head_aggregate_id {
        anyhow::bail!(
            "canonical compensation aggregate mismatch: {stale_event_id}:{head_event_id}"
        );
    }
    if persisted_head_revision != expected_head_revision
        || stale_revision >= persisted_head_revision
    {
        anyhow::bail!(
            "canonical compensation revision mismatch: stale={stale_revision}, persisted_head={persisted_head_revision}, expected_head={expected_head_revision}"
        );
    }
    let (actual_head_event_id, actual_head_revision_raw): (String, i64) = tx.query_row(
        "SELECT event_id, aggregate_revision
         FROM canonical_outbox_events
         WHERE aggregate_kind = ?1 AND aggregate_id = ?2
         ORDER BY aggregate_revision DESC LIMIT 1",
        params![head_kind, head_aggregate_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let actual_head_revision = u64::try_from(actual_head_revision_raw)
        .context("canonical compensation actual head revision is invalid")?;
    if actual_head_event_id != head_event_id || actual_head_revision_raw != expected_head_revision {
        return Err(CanonicalProjectionHeadAdvanced {
            expected_event_id: head_event_id.to_string(),
            expected_revision: head_revision,
            actual_event_id: actual_head_event_id,
            actual_revision: actual_head_revision,
        }
        .into());
    }
    let (stale_state, stale_terminal, stale_superseded_by): (
        String,
        Option<String>,
        Option<String>,
    ) = tx
        .query_row(
            "SELECT state, terminal_disposition, superseded_by_event_id
             FROM canonical_outbox_deliveries
             WHERE event_id = ?1 AND projection_target = ?2",
            params![stale_event_id, projection_target],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .with_context(|| {
            format!(
                "stale canonical outbox delivery missing for compensation: {stale_event_id}:{projection_target}"
            )
        })?;
    if stale_state == "applied"
        || stale_terminal
            .as_deref()
            .is_some_and(|terminal| !matches!(terminal, "superseded" | "compensated"))
    {
        anyhow::bail!(
            "stale canonical outbox delivery is not compensatable: {stale_event_id}:{projection_target}"
        );
    }
    if stale_terminal.is_some() {
        let prior_terminal_event_id = stale_superseded_by.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "stale canonical outbox terminal delivery lacks superseding event: {stale_event_id}:{projection_target}"
            )
        })?;
        let (prior_kind, prior_aggregate_id, prior_revision): (String, String, i64) = tx
            .query_row(
                "SELECT aggregate_kind, aggregate_id, aggregate_revision
                 FROM canonical_outbox_events WHERE event_id = ?1",
                [prior_terminal_event_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .with_context(|| {
                format!(
                    "stale canonical outbox terminal reference missing: {prior_terminal_event_id}"
                )
            })?;
        if prior_kind != head_kind
            || prior_aggregate_id != head_aggregate_id
            || prior_revision > persisted_head_revision
        {
            anyhow::bail!(
                "stale canonical outbox terminal reference is ahead or cross-aggregate: {prior_terminal_event_id}"
            );
        }
    }
    let (head_state, head_terminal): (String, Option<String>) = tx
        .query_row(
            "SELECT state, terminal_disposition
             FROM canonical_outbox_deliveries
             WHERE event_id = ?1 AND projection_target = ?2",
            params![head_event_id, projection_target],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .with_context(|| {
            format!(
                "canonical head delivery missing for compensation: {head_event_id}:{projection_target}"
            )
        })?;
    if head_terminal.is_some() {
        anyhow::bail!(
            "canonical head delivery is terminal before compensation: {head_event_id}:{projection_target}"
        );
    }
    let now = Utc::now().to_rfc3339();
    let stale_changed = tx.execute(
        "UPDATE canonical_outbox_deliveries
         SET terminal_disposition = 'compensated',
             superseded_by_event_id = ?3,
             attempt_count = attempt_count + 1,
             last_error_digest = NULL,
             updated_at = ?4
         WHERE event_id = ?1 AND projection_target = ?2
           AND state != 'applied'
           AND (
               terminal_disposition IS NULL OR
               terminal_disposition IN ('superseded', 'compensated')
           )",
        params![stale_event_id, projection_target, head_event_id, now],
    )?;
    if stale_changed == 0 {
        anyhow::bail!(
            "stale canonical outbox delivery missing for compensation: {stale_event_id}:{projection_target}"
        );
    }
    let head_changed = tx.execute(
        "UPDATE canonical_outbox_deliveries
         SET state = 'applied', attempt_count = attempt_count + 1,
             last_error_digest = NULL, updated_at = ?3
         WHERE event_id = ?1 AND projection_target = ?2 AND state != 'applied'
           AND terminal_disposition IS NULL",
        params![head_event_id, projection_target, now],
    )?;
    if head_changed == 0 {
        if head_state != "applied" {
            anyhow::bail!(
                "canonical head delivery missing for compensation: {head_event_id}:{projection_target}"
            );
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn projection_summary(conn: &Connection, event_id: &str) -> Result<ProjectionSummary> {
    let (pending, degraded, applied, superseded, compensated) = conn.query_row(
        "SELECT
            SUM(CASE WHEN state = 'pending' AND terminal_disposition IS NULL THEN 1 ELSE 0 END),
            SUM(CASE WHEN state = 'degraded' AND terminal_disposition IS NULL THEN 1 ELSE 0 END),
            SUM(CASE WHEN state = 'applied' THEN 1 ELSE 0 END),
            SUM(CASE WHEN terminal_disposition = 'superseded' THEN 1 ELSE 0 END),
            SUM(CASE WHEN terminal_disposition = 'compensated' THEN 1 ELSE 0 END)
         FROM canonical_outbox_deliveries WHERE event_id = ?1",
        [event_id],
        |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                row.get::<_, Option<i64>>(4)?.unwrap_or(0),
            ))
        },
    )?;
    Ok(ProjectionSummary {
        event_id: event_id.to_string(),
        pending: u32::try_from(pending).context("negative pending projection count")?,
        degraded: u32::try_from(degraded).context("negative degraded projection count")?,
        applied: u32::try_from(applied).context("negative applied projection count")?,
        superseded: u32::try_from(superseded).context("negative superseded projection count")?,
        compensated: u32::try_from(compensated).context("negative compensated projection count")?,
    })
}

fn delivery_state(
    conn: &Connection,
    event_id: &str,
    projection_target: &str,
) -> Result<Option<ProjectionDeliveryState>> {
    conn.query_row(
        "SELECT state FROM canonical_outbox_deliveries
         WHERE event_id = ?1 AND projection_target = ?2",
        params![event_id, projection_target],
        |row| row.get::<_, String>(0),
    )
    .optional()?
    .map(|state| ProjectionDeliveryState::parse(&state))
    .transpose()
}

fn row_to_receipt(row: &rusqlite::Row<'_>) -> rusqlite::Result<CanonicalMutationReceipt> {
    let aggregate_revision_raw: i64 = row.get(4)?;
    let aggregate_revision = u64::try_from(aggregate_revision_raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })?;
    let created_at = parse_timestamp(row, 7)?;
    Ok(CanonicalMutationReceipt {
        event_id: row.get(0)?,
        aggregate_kind: row.get(1)?,
        aggregate_id: row.get(2)?,
        mutation_kind: row.get(3)?,
        aggregate_revision,
        payload_digest: row.get(5)?,
        tombstone_id: row.get(6)?,
        created_at,
    })
}

fn row_to_delivery(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectionDelivery> {
    let aggregate_revision_raw: i64 = row.get(4)?;
    let aggregate_revision = u64::try_from(aggregate_revision_raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })?;
    let state_raw: String = row.get(8)?;
    let state = ProjectionDeliveryState::parse(&state_raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            8,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.to_string(),
            )),
        )
    })?;
    let attempt_count_raw: i64 = row.get(9)?;
    let attempt_count = u32::try_from(attempt_count_raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            9,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })?;
    Ok(ProjectionDelivery {
        event_id: row.get(0)?,
        aggregate_kind: row.get(1)?,
        aggregate_id: row.get(2)?,
        mutation_kind: row.get(3)?,
        aggregate_revision,
        payload_digest: row.get(5)?,
        tombstone_id: row.get(6)?,
        projection_target: row.get(7)?,
        state,
        attempt_count,
        last_error_digest: row.get(10)?,
        terminal_disposition: row.get(11)?,
        superseded_by_event_id: row.get(12)?,
        created_at: parse_timestamp(row, 13)?,
        updated_at: parse_timestamp(row, 14)?,
    })
}

fn parse_timestamp(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<DateTime<Utc>> {
    let raw: String = row.get(index)?;
    DateTime::parse_from_rfc3339(&raw)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
}

fn validate_identifier(label: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_ID_LEN
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        anyhow::bail!("invalid canonical outbox {label}");
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<()> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        anyhow::bail!("canonical outbox payload digest must be sha256");
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        anyhow::bail!("canonical outbox payload digest is malformed");
    }
    Ok(())
}

fn validate_targets(targets: &[&str]) -> Result<()> {
    if targets.len() > MAX_PROJECTION_TARGETS {
        anyhow::bail!("too many canonical outbox projection targets");
    }
    let mut unique = std::collections::HashSet::new();
    for target in targets {
        validate_identifier("projection_target", target)?;
        if !unique.insert(*target) {
            anyhow::bail!("duplicate canonical outbox projection target: {target}");
        }
    }
    Ok(())
}

/// Durable recovery journal for a canonical file owner. This is deliberately
/// a prepare/observe protocol rather than a false cross-file transaction: the
/// journal is fsynced by SQLite before the canonical atomic rename, and a
/// restart compares the observed canonical digest with `before` / `after`.
pub struct FileMutationJournal {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileMutationState {
    Prepared,
    Committed,
    NotCommitted,
    Degraded,
}

impl FileMutationState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Committed => "committed",
            Self::NotCommitted => "not_committed",
            Self::Degraded => "degraded",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "prepared" => Ok(Self::Prepared),
            "committed" => Ok(Self::Committed),
            "not_committed" => Ok(Self::NotCommitted),
            "degraded" => Ok(Self::Degraded),
            other => anyhow::bail!("unsupported file mutation state: {other}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMutationReceipt {
    pub operation_id: String,
    pub aggregate_kind: String,
    pub aggregate_id: String,
    pub mutation_kind: String,
    pub before_digest: String,
    pub after_digest: String,
    pub state: FileMutationState,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileProjectionDelivery {
    pub operation_id: String,
    pub aggregate_kind: String,
    pub aggregate_id: String,
    pub mutation_kind: String,
    pub before_digest: String,
    pub after_digest: String,
    pub projection_target: String,
    pub state: ProjectionDeliveryState,
    pub attempt_count: u32,
    pub last_error_digest: Option<String>,
}

impl FileMutationJournal {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("create canonical file journal parent {}", parent.display())
            })?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("open canonical file journal {}", path.display()))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS canonical_file_mutations (
                operation_id TEXT PRIMARY KEY,
                aggregate_kind TEXT NOT NULL,
                aggregate_id TEXT NOT NULL,
                mutation_kind TEXT NOT NULL,
                before_digest TEXT NOT NULL,
                after_digest TEXT NOT NULL,
                state TEXT NOT NULL CHECK(state IN (
                    'prepared', 'committed', 'not_committed', 'degraded'
                )),
                last_error_digest TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_canonical_file_mutation_state
             ON canonical_file_mutations(state, updated_at);
             CREATE TABLE IF NOT EXISTS canonical_file_active_operations (
                aggregate_kind TEXT NOT NULL,
                aggregate_id TEXT NOT NULL,
                operation_id TEXT NOT NULL UNIQUE,
                PRIMARY KEY(aggregate_kind, aggregate_id),
                FOREIGN KEY(operation_id) REFERENCES canonical_file_mutations(operation_id)
                    ON DELETE CASCADE
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS canonical_file_projection_deliveries (
                operation_id TEXT NOT NULL,
                projection_target TEXT NOT NULL,
                state TEXT NOT NULL CHECK(state IN ('pending', 'degraded', 'applied')),
                attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count >= 0),
                last_error_digest TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY(operation_id, projection_target),
                FOREIGN KEY(operation_id) REFERENCES canonical_file_mutations(operation_id)
                    ON DELETE CASCADE
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS idx_canonical_file_projection_queue
             ON canonical_file_projection_deliveries(state, updated_at, operation_id);",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn prepare(
        &self,
        aggregate_kind: &str,
        aggregate_id: &str,
        mutation_kind: &str,
        before_digest: &str,
        after_digest: &str,
        projection_targets: &[&str],
    ) -> Result<FileMutationReceipt> {
        validate_identifier("aggregate_kind", aggregate_kind)?;
        validate_identifier("aggregate_id", aggregate_id)?;
        validate_identifier("mutation_kind", mutation_kind)?;
        validate_digest(before_digest)?;
        validate_digest(after_digest)?;
        if before_digest == after_digest {
            anyhow::bail!("canonical file mutation before and after digests are equal");
        }
        validate_targets(projection_targets)?;
        let now = Utc::now();
        let operation_id = format!("file-outbox:{}", Uuid::new_v4());
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let unresolved: Option<String> = tx
            .query_row(
                "SELECT mutations.operation_id
                 FROM canonical_file_mutations mutations
                 WHERE mutations.aggregate_kind = ?1
                   AND mutations.aggregate_id = ?2
                   AND (
                       mutations.state IN ('prepared', 'degraded')
                       OR (
                           mutations.state = 'committed'
                           AND EXISTS (
                               SELECT 1 FROM canonical_file_projection_deliveries deliveries
                               WHERE deliveries.operation_id = mutations.operation_id
                                 AND deliveries.state IN ('pending', 'degraded')
                           )
                       )
                   )
                 ORDER BY mutations.created_at ASC
                 LIMIT 1",
                params![aggregate_kind, aggregate_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(unresolved) = unresolved {
            anyhow::bail!("canonical file mutation blocked by unresolved operation {unresolved}");
        }
        tx.execute(
            "INSERT INTO canonical_file_mutations (
                operation_id, aggregate_kind, aggregate_id, mutation_kind,
                before_digest, after_digest, state, last_error_digest,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'prepared', NULL, ?7, ?7)",
            params![
                operation_id,
                aggregate_kind,
                aggregate_id,
                mutation_kind,
                before_digest,
                after_digest,
                now.to_rfc3339()
            ],
        )?;
        tx.execute(
            "INSERT INTO canonical_file_active_operations (
                aggregate_kind, aggregate_id, operation_id
             ) VALUES (?1, ?2, ?3)",
            params![aggregate_kind, aggregate_id, operation_id],
        )?;
        for target in projection_targets {
            tx.execute(
                "INSERT INTO canonical_file_projection_deliveries (
                    operation_id, projection_target, state, attempt_count,
                    last_error_digest, created_at, updated_at
                 ) VALUES (?1, ?2, 'pending', 0, NULL, ?3, ?3)",
                params![operation_id, target, now.to_rfc3339()],
            )?;
        }
        tx.commit()?;
        Ok(FileMutationReceipt {
            operation_id,
            aggregate_kind: aggregate_kind.to_string(),
            aggregate_id: aggregate_id.to_string(),
            mutation_kind: mutation_kind.to_string(),
            before_digest: before_digest.to_string(),
            after_digest: after_digest.to_string(),
            state: FileMutationState::Prepared,
            created_at: now,
        })
    }

    /// Reconcile a prepared file mutation against the canonical file digest.
    /// `after` proves commit; `before` proves the atomic rename did not occur;
    /// anything else remains degraded/unknown and is never projected.
    pub fn observe_canonical_digest(
        &self,
        operation_id: &str,
        observed_digest: &str,
    ) -> Result<FileMutationState> {
        validate_digest(observed_digest)?;
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let (before_digest, after_digest, current_state): (String, String, String) = tx.query_row(
            "SELECT before_digest, after_digest, state
                 FROM canonical_file_mutations WHERE operation_id = ?1",
            [operation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let current_state = FileMutationState::parse(&current_state)?;
        let next_state = if observed_digest == after_digest {
            FileMutationState::Committed
        } else if observed_digest == before_digest && current_state == FileMutationState::Prepared {
            FileMutationState::NotCommitted
        } else {
            FileMutationState::Degraded
        };
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE canonical_file_mutations
             SET state = ?2,
                 last_error_digest = CASE WHEN ?2 = 'degraded' THEN ?3 ELSE NULL END,
                 updated_at = ?4
             WHERE operation_id = ?1",
            params![
                operation_id,
                next_state.as_str(),
                (next_state == FileMutationState::Degraded)
                    .then(|| metadata_digest("canonical_file_digest_diverged")),
                now
            ],
        )?;
        if next_state == FileMutationState::NotCommitted {
            tx.execute(
                "UPDATE canonical_file_projection_deliveries
                 SET state = 'applied', attempt_count = attempt_count + 1,
                     last_error_digest = NULL, updated_at = ?2
                 WHERE operation_id = ?1 AND state != 'applied'",
                params![operation_id, now],
            )?;
        } else if next_state == FileMutationState::Degraded {
            tx.execute(
                "UPDATE canonical_file_projection_deliveries
                 SET state = 'degraded', attempt_count = attempt_count + 1,
                     last_error_digest = ?2, updated_at = ?3
                 WHERE operation_id = ?1 AND state != 'applied'",
                params![
                    operation_id,
                    metadata_digest("canonical_file_digest_diverged"),
                    now
                ],
            )?;
        }
        if next_state == FileMutationState::NotCommitted
            || (next_state == FileMutationState::Committed
                && tx.query_row(
                    "SELECT COUNT(*) FROM canonical_file_projection_deliveries
                     WHERE operation_id = ?1 AND state != 'applied'",
                    [operation_id],
                    |row| row.get::<_, i64>(0),
                )? == 0)
        {
            tx.execute(
                "DELETE FROM canonical_file_active_operations WHERE operation_id = ?1",
                [operation_id],
            )?;
        }
        tx.commit()?;
        Ok(next_state)
    }

    /// Record that the guarded compare-and-swap rejected the mutation before
    /// the canonical file write was attempted. This explicit transition is
    /// stronger evidence than comparing against a file that another writer may
    /// already have advanced.
    pub fn mark_not_committed(&self, operation_id: &str) -> Result<()> {
        validate_identifier("operation_id", operation_id)?;
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let current: String = tx.query_row(
            "SELECT state FROM canonical_file_mutations WHERE operation_id = ?1",
            [operation_id],
            |row| row.get(0),
        )?;
        match FileMutationState::parse(&current)? {
            FileMutationState::Prepared | FileMutationState::NotCommitted => {}
            FileMutationState::Committed => {
                anyhow::bail!("committed canonical file mutation cannot become not_committed")
            }
            FileMutationState::Degraded => {
                anyhow::bail!("degraded canonical file mutation requires digest reconciliation")
            }
        }
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE canonical_file_mutations
             SET state = 'not_committed', last_error_digest = NULL, updated_at = ?2
             WHERE operation_id = ?1",
            params![operation_id, now],
        )?;
        tx.execute(
            "UPDATE canonical_file_projection_deliveries
             SET state = 'applied', attempt_count = attempt_count + 1,
                 last_error_digest = NULL, updated_at = ?2
             WHERE operation_id = ?1 AND state != 'applied'",
            params![operation_id, now],
        )?;
        tx.execute(
            "DELETE FROM canonical_file_active_operations WHERE operation_id = ?1",
            [operation_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Preserve an uncertain durability result without projecting it. A later
    /// restart may reconcile the operation by comparing the canonical digest
    /// with the prepared before/after digests.
    pub fn mark_degraded(&self, operation_id: &str, error: &str) -> Result<()> {
        validate_identifier("operation_id", operation_id)?;
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let current: String = tx.query_row(
            "SELECT state FROM canonical_file_mutations WHERE operation_id = ?1",
            [operation_id],
            |row| row.get(0),
        )?;
        if FileMutationState::parse(&current)? == FileMutationState::NotCommitted {
            anyhow::bail!("not_committed canonical file mutation cannot become degraded");
        }
        let now = Utc::now().to_rfc3339();
        let error_digest = metadata_digest(error);
        tx.execute(
            "UPDATE canonical_file_mutations
             SET state = 'degraded', last_error_digest = ?2, updated_at = ?3
             WHERE operation_id = ?1",
            params![operation_id, error_digest, now],
        )?;
        tx.execute(
            "UPDATE canonical_file_projection_deliveries
             SET state = 'degraded', attempt_count = attempt_count + 1,
                 last_error_digest = ?2, updated_at = ?3
             WHERE operation_id = ?1 AND state != 'applied'",
            params![operation_id, error_digest, now],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn reconcile_prepared(
        &self,
        observed_digest: &str,
    ) -> Result<Vec<(String, FileMutationState)>> {
        let operation_ids = {
            let conn = self.lock()?;
            let mut statement = conn.prepare(
                "SELECT operation_id FROM canonical_file_mutations
                 WHERE state IN ('prepared', 'degraded') ORDER BY created_at ASC",
            )?;
            let ids = statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            ids
        };
        operation_ids
            .into_iter()
            .map(|operation_id| {
                self.observe_canonical_digest(&operation_id, observed_digest)
                    .map(|state| (operation_id, state))
            })
            .collect()
    }

    pub fn list_replayable_deliveries(&self, limit: usize) -> Result<Vec<FileProjectionDelivery>> {
        let conn = self.lock()?;
        let mut statement = conn.prepare(
            "SELECT mutations.operation_id, mutations.aggregate_kind,
                    mutations.aggregate_id, mutations.mutation_kind,
                    mutations.before_digest, mutations.after_digest,
                    deliveries.projection_target, deliveries.state,
                    deliveries.attempt_count, deliveries.last_error_digest
             FROM canonical_file_projection_deliveries deliveries
             JOIN canonical_file_mutations mutations
               ON mutations.operation_id = deliveries.operation_id
             WHERE mutations.state = 'committed'
               AND deliveries.state IN ('pending', 'degraded')
             ORDER BY deliveries.updated_at ASC, deliveries.operation_id ASC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([limit.clamp(1, 500) as i64], |row| {
            let state_raw: String = row.get(7)?;
            let state = ProjectionDeliveryState::parse(&state_raw).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        error.to_string(),
                    )),
                )
            })?;
            let attempts = u32::try_from(row.get::<_, i64>(8)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    8,
                    rusqlite::types::Type::Integer,
                    Box::new(error),
                )
            })?;
            Ok(FileProjectionDelivery {
                operation_id: row.get(0)?,
                aggregate_kind: row.get(1)?,
                aggregate_id: row.get(2)?,
                mutation_kind: row.get(3)?,
                before_digest: row.get(4)?,
                after_digest: row.get(5)?,
                projection_target: row.get(6)?,
                state,
                attempt_count: attempts,
                last_error_digest: row.get(9)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn mark_projection_applied(&self, operation_id: &str, target: &str) -> Result<()> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let changed = tx.execute(
            "UPDATE canonical_file_projection_deliveries
             SET state = 'applied', attempt_count = attempt_count + 1,
                 last_error_digest = NULL, updated_at = ?3
             WHERE operation_id = ?1 AND projection_target = ?2 AND state != 'applied'",
            params![operation_id, target, Utc::now().to_rfc3339()],
        )?;
        if changed == 0 {
            let already_applied = tx
                .query_row(
                    "SELECT 1 FROM canonical_file_projection_deliveries
                     WHERE operation_id = ?1 AND projection_target = ?2 AND state = 'applied'",
                    params![operation_id, target],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !already_applied {
                anyhow::bail!("canonical file projection delivery missing");
            }
        }
        let remaining: i64 = tx.query_row(
            "SELECT COUNT(*) FROM canonical_file_projection_deliveries
             WHERE operation_id = ?1 AND state != 'applied'",
            [operation_id],
            |row| row.get(0),
        )?;
        if remaining == 0 {
            tx.execute(
                "DELETE FROM canonical_file_active_operations WHERE operation_id = ?1",
                [operation_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn mark_projection_degraded(
        &self,
        operation_id: &str,
        target: &str,
        error: &str,
    ) -> Result<()> {
        let conn = self.lock()?;
        let changed = conn.execute(
            "UPDATE canonical_file_projection_deliveries
             SET state = 'degraded', attempt_count = attempt_count + 1,
                 last_error_digest = ?3, updated_at = ?4
             WHERE operation_id = ?1 AND projection_target = ?2 AND state != 'applied'",
            params![
                operation_id,
                target,
                metadata_digest(error),
                Utc::now().to_rfc3339()
            ],
        )?;
        if changed == 0 {
            anyhow::bail!("canonical file projection delivery missing or already applied");
        }
        Ok(())
    }

    pub fn operation_state(&self, operation_id: &str) -> Result<FileMutationState> {
        let conn = self.lock()?;
        let state: String = conn.query_row(
            "SELECT state FROM canonical_file_mutations WHERE operation_id = ?1",
            [operation_id],
            |row| row.get(0),
        )?;
        FileMutationState::parse(&state)
    }

    pub fn unresolved_operation(
        &self,
        aggregate_kind: &str,
        aggregate_id: &str,
    ) -> Result<Option<(String, FileMutationState)>> {
        validate_identifier("aggregate_kind", aggregate_kind)?;
        validate_identifier("aggregate_id", aggregate_id)?;
        let conn = self.lock()?;
        let unresolved: Option<(String, String)> = conn
            .query_row(
                "SELECT mutations.operation_id, mutations.state
                 FROM canonical_file_mutations mutations
                 WHERE mutations.aggregate_kind = ?1
                   AND mutations.aggregate_id = ?2
                   AND (
                       mutations.state IN ('prepared', 'degraded')
                       OR (
                           mutations.state = 'committed'
                           AND EXISTS (
                               SELECT 1 FROM canonical_file_projection_deliveries deliveries
                               WHERE deliveries.operation_id = mutations.operation_id
                                 AND deliveries.state IN ('pending', 'degraded')
                           )
                       )
                   )
                 ORDER BY mutations.created_at ASC
                 LIMIT 1",
                params![aggregate_kind, aggregate_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        unresolved
            .map(|(operation_id, state)| {
                FileMutationState::parse(&state).map(|state| (operation_id, state))
            })
            .transpose()
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|error| anyhow::anyhow!("file mutation journal mutex poison: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn tombstone_and_delivery_rows_share_the_canonical_transaction() {
        let mut conn = store();
        {
            let tx = conn.transaction().unwrap();
            tx.execute(
                "CREATE TABLE canonical_records (id TEXT PRIMARY KEY, content TEXT NOT NULL)",
                [],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO canonical_records VALUES ('session-1', 'private sentinel')",
                [],
            )
            .unwrap();
            tx.commit().unwrap();
        }

        let injected_failure = {
            let tx = conn.transaction().unwrap();
            tx.execute("DELETE FROM canonical_records WHERE id = 'session-1'", [])
                .unwrap();
            enqueue_tombstone(
                &tx,
                "conversation",
                "session-1",
                Some("user requested deletion"),
                &["vector_store"],
            )
            .unwrap();
            tx.execute("INSERT INTO missing_table VALUES (1)", []).err()
        };
        assert!(injected_failure.is_some());
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM canonical_records WHERE id = 'session-1'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM canonical_outbox_events", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0
        );
    }

    #[test]
    fn duplicate_delivery_is_idempotent_and_degraded_delivery_is_replayable() {
        let mut conn = store();
        let receipt = {
            let tx = conn.transaction().unwrap();
            let receipt =
                enqueue_tombstone(&tx, "conversation", "session-1", None, &["vector_store"])
                    .unwrap();
            tx.commit().unwrap();
            receipt
        };
        mark_delivery_degraded(
            &conn,
            &receipt.event_id,
            "vector_store",
            "disk contained private sentinel but write failed",
        )
        .unwrap();
        let replay = list_replayable_deliveries(&conn, 10).unwrap();
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].state, ProjectionDeliveryState::Degraded);
        assert!(!replay[0]
            .last_error_digest
            .as_deref()
            .unwrap()
            .contains("private sentinel"));

        mark_delivery_applied(&conn, &receipt.event_id, "vector_store").unwrap();
        mark_delivery_applied(&conn, &receipt.event_id, "vector_store").unwrap();
        assert!(list_replayable_deliveries(&conn, 10).unwrap().is_empty());
        assert_eq!(
            projection_summary(&conn, &receipt.event_id)
                .unwrap()
                .state(),
            ProjectionDeliveryState::Applied
        );
    }

    #[test]
    fn exact_event_replay_never_drains_unrelated_backlog() {
        let mut conn = store();
        let (foreground, unrelated) = {
            let tx = conn.transaction().unwrap();
            let foreground = enqueue_mutation(
                &tx,
                "memory_lifecycle",
                "memory:foreground",
                "materialized",
                &metadata_digest("foreground-token"),
                &["memory_store", "vector_store"],
            )
            .unwrap();
            let unrelated = enqueue_mutation(
                &tx,
                "memory_lifecycle",
                "memory:unrelated",
                "materialized",
                &metadata_digest("unrelated-token"),
                &["vector_store"],
            )
            .unwrap();
            tx.commit().unwrap();
            (foreground, unrelated)
        };
        mark_delivery_applied(&conn, &foreground.event_id, "memory_store").unwrap();
        mark_delivery_degraded(
            &conn,
            &foreground.event_id,
            "vector_store",
            "provider unavailable",
        )
        .unwrap();

        let exact = list_replayable_deliveries_for_event(&conn, &foreground.event_id).unwrap();
        assert_eq!(exact.len(), 1);
        assert_eq!(exact[0].event_id, foreground.event_id);
        assert_eq!(exact[0].projection_target, "vector_store");
        assert_eq!(exact[0].state, ProjectionDeliveryState::Degraded);
        assert!(exact
            .iter()
            .all(|delivery| delivery.event_id != unrelated.event_id));

        let all = list_replayable_deliveries(&conn, 10).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all
            .iter()
            .any(|delivery| delivery.event_id == unrelated.event_id));
    }

    #[test]
    fn repeated_tombstone_reuses_one_canonical_deletion_fact() {
        let mut conn = store();
        let tx = conn.transaction().unwrap();
        let first =
            enqueue_tombstone(&tx, "agent_run", "run-1", None, &["turn_event_store"]).unwrap();
        tx.commit().unwrap();

        let tx = conn.transaction().unwrap();
        let second = enqueue_tombstone(
            &tx,
            "agent_run",
            "run-1",
            Some("different prose must not duplicate deletion"),
            &["turn_event_store", "life_event_store"],
        )
        .unwrap();
        tx.commit().unwrap();
        assert_eq!(first.event_id, second.event_id);
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM canonical_tombstones", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            1
        );
        assert_eq!(list_replayable_deliveries(&conn, 10).unwrap().len(), 2);
    }

    #[test]
    fn restore_supersedes_old_delete_queue_and_exposes_metadata_fence() {
        let mut conn = store();
        let deleted = {
            let tx = conn.transaction().unwrap();
            let receipt = enqueue_tombstone(
                &tx,
                "agent_run",
                "run-restored",
                None,
                &["turn_event_store", "action_queue_store", "life_event_store"],
            )
            .unwrap();
            tx.commit().unwrap();
            receipt
        };
        mark_delivery_degraded(
            &conn,
            &deleted.event_id,
            "life_event_store",
            "injected projection failure",
        )
        .unwrap();

        let restored = {
            let tx = conn.transaction().unwrap();
            let receipt = supersede_active_tombstone(
                &tx,
                "agent_run",
                "run-restored",
                &metadata_digest("restored"),
                &["turn_event_store", "action_queue_store", "life_event_store"],
            )
            .unwrap();
            tx.commit().unwrap();
            receipt
        };

        assert_eq!(deleted.aggregate_revision, 1);
        assert_eq!(restored.aggregate_revision, 2);

        assert_eq!(
            superseded_tombstone_ids_for_event(&conn, &restored.event_id).unwrap(),
            vec![deleted.tombstone_id.clone().unwrap()]
        );
        assert!(
            list_replayable_deliveries_for_event(&conn, &deleted.event_id)
                .unwrap()
                .is_empty()
        );
        let replayable = list_replayable_deliveries(&conn, 20).unwrap();
        assert_eq!(replayable.len(), 3);
        assert!(replayable
            .iter()
            .all(|delivery| delivery.event_id == restored.event_id));
        assert!(replayable
            .iter()
            .all(|delivery| delivery.aggregate_revision == 2));
        let deleted_summary = projection_summary(&conn, &deleted.event_id).unwrap();
        assert_eq!(deleted_summary.pending, 0);
        assert_eq!(deleted_summary.degraded, 0);
        assert_eq!(deleted_summary.superseded, 3);
        assert_eq!(deleted_summary.state(), ProjectionDeliveryState::Superseded);
    }

    #[test]
    fn stale_head_finalization_is_typed_and_never_clears_newer_supersede_truth() {
        let mut conn = store();
        let deleted_first = {
            let tx = conn.transaction().unwrap();
            let receipt =
                enqueue_tombstone(&tx, "agent_run", "run-fourth", None, &["turn"]).unwrap();
            tx.commit().unwrap();
            receipt
        };
        mark_delivery_applied(&conn, &deleted_first.event_id, "turn").unwrap();
        let restored_stale = {
            let tx = conn.transaction().unwrap();
            let receipt = supersede_active_tombstone(
                &tx,
                "agent_run",
                "run-fourth",
                &metadata_digest("restore-one"),
                &["turn"],
            )
            .unwrap();
            tx.commit().unwrap();
            receipt
        };
        let deleted_stale_head = {
            let tx = conn.transaction().unwrap();
            let receipt =
                enqueue_tombstone(&tx, "agent_run", "run-fourth", None, &["turn"]).unwrap();
            tx.commit().unwrap();
            receipt
        };
        let restored_current = {
            let tx = conn.transaction().unwrap();
            let receipt = supersede_active_tombstone(
                &tx,
                "agent_run",
                "run-fourth",
                &metadata_digest("restore-two"),
                &["turn"],
            )
            .unwrap();
            tx.commit().unwrap();
            receipt
        };
        assert_eq!(restored_stale.aggregate_revision, 2);
        assert_eq!(deleted_stale_head.aggregate_revision, 3);
        assert_eq!(restored_current.aggregate_revision, 4);

        let compensation_error = mark_delivery_compensated_to_head(
            &mut conn,
            &restored_stale.event_id,
            &deleted_stale_head.event_id,
            deleted_stale_head.aggregate_revision,
            "turn",
        )
        .unwrap_err();
        let advanced = compensation_error
            .downcast_ref::<CanonicalProjectionHeadAdvanced>()
            .expect("head advance must remain machine distinguishable");
        assert_eq!(advanced.actual_event_id, restored_current.event_id);
        assert_eq!(advanced.actual_revision, 4);

        let applied_error = mark_delivery_applied_if_canonical_head(
            &mut conn,
            &deleted_stale_head.event_id,
            deleted_stale_head.aggregate_revision,
            "turn",
        )
        .unwrap_err();
        assert!(applied_error
            .downcast_ref::<CanonicalProjectionHeadAdvanced>()
            .is_some());
        let (terminal, superseded_by): (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT terminal_disposition, superseded_by_event_id
                 FROM canonical_outbox_deliveries
                 WHERE event_id = ?1 AND projection_target = 'turn'",
                [&deleted_stale_head.event_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(terminal.as_deref(), Some("superseded"));
        assert_eq!(
            superseded_by.as_deref(),
            Some(restored_current.event_id.as_str())
        );
    }

    #[test]
    fn file_journal_distinguishes_crash_before_and_after_atomic_rename() {
        let directory = tempfile::tempdir().unwrap();
        let journal = FileMutationJournal::new(directory.path().join("journal.db")).unwrap();
        let before = metadata_digest("before");
        let after = metadata_digest("after");

        let not_committed = journal
            .prepare(
                "lifemodel",
                "current",
                "updated",
                &before,
                &after,
                &["daily_snapshot"],
            )
            .unwrap();
        assert_eq!(
            journal
                .observe_canonical_digest(&not_committed.operation_id, &before)
                .unwrap(),
            FileMutationState::NotCommitted
        );
        assert!(journal.list_replayable_deliveries(10).unwrap().is_empty());

        let committed = journal
            .prepare(
                "lifemodel",
                "current",
                "updated",
                &before,
                &after,
                &["daily_snapshot"],
            )
            .unwrap();
        assert_eq!(
            journal
                .observe_canonical_digest(&committed.operation_id, &after)
                .unwrap(),
            FileMutationState::Committed
        );
        assert_eq!(journal.list_replayable_deliveries(10).unwrap().len(), 1);
        journal
            .mark_projection_applied(&committed.operation_id, "daily_snapshot")
            .unwrap();
        journal
            .mark_projection_applied(&committed.operation_id, "daily_snapshot")
            .unwrap();
        assert!(journal.list_replayable_deliveries(10).unwrap().is_empty());
    }

    #[test]
    fn file_journal_preserves_unknown_on_unrelated_observed_digest() {
        let directory = tempfile::tempdir().unwrap();
        let journal = FileMutationJournal::new(directory.path().join("journal.db")).unwrap();
        let receipt = journal
            .prepare(
                "lifemodel",
                "current",
                "updated",
                &metadata_digest("before"),
                &metadata_digest("after"),
                &["daily_snapshot"],
            )
            .unwrap();
        assert_eq!(
            journal
                .observe_canonical_digest(&receipt.operation_id, &metadata_digest("third-state"))
                .unwrap(),
            FileMutationState::Degraded
        );
        assert!(journal.list_replayable_deliveries(10).unwrap().is_empty());
    }

    #[test]
    fn file_journal_records_pre_write_cas_rejection_without_observing_newer_file() {
        let directory = tempfile::tempdir().unwrap();
        let journal = FileMutationJournal::new(directory.path().join("journal.db")).unwrap();
        let receipt = journal
            .prepare(
                "lifemodel",
                "current",
                "updated",
                &metadata_digest("before"),
                &metadata_digest("after"),
                &["patch_store"],
            )
            .unwrap();

        journal.mark_not_committed(&receipt.operation_id).unwrap();
        journal.mark_not_committed(&receipt.operation_id).unwrap();
        assert_eq!(
            journal.operation_state(&receipt.operation_id).unwrap(),
            FileMutationState::NotCommitted
        );
        assert!(journal.list_replayable_deliveries(10).unwrap().is_empty());
    }

    #[test]
    fn file_journal_degraded_write_can_reconcile_after_restart() {
        let directory = tempfile::tempdir().unwrap();
        let journal = FileMutationJournal::new(directory.path().join("journal.db")).unwrap();
        let after = metadata_digest("after");
        let receipt = journal
            .prepare(
                "lifemodel",
                "current",
                "updated",
                &metadata_digest("before"),
                &after,
                &["patch_store"],
            )
            .unwrap();

        journal
            .mark_degraded(
                &receipt.operation_id,
                "parent directory fsync result unknown",
            )
            .unwrap();
        assert_eq!(
            journal.operation_state(&receipt.operation_id).unwrap(),
            FileMutationState::Degraded
        );
        assert_eq!(
            journal
                .observe_canonical_digest(&receipt.operation_id, &after)
                .unwrap(),
            FileMutationState::Committed
        );
        assert_eq!(journal.list_replayable_deliveries(10).unwrap().len(), 1);
    }

    #[test]
    fn file_journal_blocks_next_write_until_prior_projection_is_acknowledged() {
        let directory = tempfile::tempdir().unwrap();
        let journal = FileMutationJournal::new(directory.path().join("journal.db")).unwrap();
        let first = journal
            .prepare(
                "lifemodel",
                "current",
                "updated",
                &metadata_digest("before"),
                &metadata_digest("after"),
                &["patch_store"],
            )
            .unwrap();
        assert!(journal
            .prepare(
                "lifemodel",
                "current",
                "updated",
                &metadata_digest("before"),
                &metadata_digest("other-after"),
                &["patch_store"],
            )
            .is_err());

        journal
            .observe_canonical_digest(&first.operation_id, &metadata_digest("after"))
            .unwrap();
        assert!(journal
            .prepare(
                "lifemodel",
                "current",
                "updated",
                &metadata_digest("after"),
                &metadata_digest("next"),
                &["patch_store"],
            )
            .is_err());
        journal
            .mark_projection_applied(&first.operation_id, "patch_store")
            .unwrap();
        assert!(journal
            .prepare(
                "lifemodel",
                "current",
                "updated",
                &metadata_digest("after"),
                &metadata_digest("next"),
                &["patch_store"],
            )
            .is_ok());
    }
}
