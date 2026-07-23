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

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
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
    if head_changed == 0 && head_state != "applied" {
        anyhow::bail!(
            "canonical head delivery missing for compensation: {head_event_id}:{projection_target}"
        );
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

/// Durable, metadata-only saga journal for the governed multi-owner data
/// import. This journal records no LifeModel, message, vector, or StateStore
/// body. It also deliberately makes no cross-database atomicity claim: each
/// owner remains responsible for its own canonical transaction, while this
/// journal makes an interrupted sequence observable and fail-closed after a
/// restart.
pub struct GovernedDataImportJournal {
    conn: Mutex<Connection>,
}

pub const GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON: &str = "data_import_recovery_required";
const MAX_GOVERNED_IMPORT_OWNERS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernedDataImportStage {
    Prepared,
    LifeModelApplied,
    MemoryApplied,
    VectorApplied,
    StateCommitted,
    ProjectionDegraded,
    Completed,
    Compensated,
    CompensationUnknown,
    /// The original payload is unavailable, so recovery cannot safely replay
    /// or compensate the saga. Every canonical owner has instead been
    /// re-observed and the operation is terminalized while preserving those
    /// current owner facts. This is deliberately distinct from both
    /// `completed` and `compensated`.
    AbandonedPreservingCurrent,
}

impl GovernedDataImportStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::LifeModelApplied => "lifemodel_applied",
            Self::MemoryApplied => "memory_applied",
            Self::VectorApplied => "vector_applied",
            Self::StateCommitted => "state_committed",
            Self::ProjectionDegraded => "projection_degraded",
            Self::Completed => "completed",
            Self::Compensated => "compensated",
            Self::CompensationUnknown => "compensation_unknown",
            Self::AbandonedPreservingCurrent => "abandoned_preserving_current",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Compensated | Self::AbandonedPreservingCurrent
        )
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "prepared" => Ok(Self::Prepared),
            "lifemodel_applied" => Ok(Self::LifeModelApplied),
            "memory_applied" => Ok(Self::MemoryApplied),
            "vector_applied" => Ok(Self::VectorApplied),
            "state_committed" => Ok(Self::StateCommitted),
            "projection_degraded" => Ok(Self::ProjectionDegraded),
            "completed" => Ok(Self::Completed),
            "compensated" => Ok(Self::Compensated),
            "compensation_unknown" => Ok(Self::CompensationUnknown),
            "abandoned_preserving_current" => Ok(Self::AbandonedPreservingCurrent),
            other => anyhow::bail!("unsupported governed data-import stage: {other}"),
        }
    }

    fn may_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        match (self, next) {
            (Self::Prepared, Self::LifeModelApplied)
            | (Self::LifeModelApplied, Self::MemoryApplied)
            | (Self::MemoryApplied, Self::VectorApplied)
            | (Self::VectorApplied, Self::StateCommitted)
            | (Self::StateCommitted, Self::ProjectionDegraded)
            | (Self::StateCommitted, Self::Completed)
            | (Self::ProjectionDegraded, Self::Completed)
            | (Self::CompensationUnknown, Self::Compensated) => true,
            (current, Self::Compensated | Self::CompensationUnknown) => !current.is_terminal(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernedDataImportResolutionClassification {
    Before,
    Target,
    Other,
}

impl GovernedDataImportResolutionClassification {
    fn as_str(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::Target => "target",
            Self::Other => "other",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "before" => Ok(Self::Before),
            "target" => Ok(Self::Target),
            "other" => Ok(Self::Other),
            other => {
                anyhow::bail!("unsupported governed data-import resolution classification: {other}")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernedDataImportOwnerStatus {
    Pending,
    Applied,
    Skipped,
    Compensated,
    Unknown,
}

impl GovernedDataImportOwnerStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Applied => "applied",
            Self::Skipped => "skipped",
            Self::Compensated => "compensated",
            Self::Unknown => "unknown",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "applied" => Ok(Self::Applied),
            "skipped" => Ok(Self::Skipped),
            "compensated" => Ok(Self::Compensated),
            "unknown" => Ok(Self::Unknown),
            other => anyhow::bail!("unsupported governed data-import owner status: {other}"),
        }
    }

    fn may_transition_to(self, next: Self) -> bool {
        self == next
            || matches!(
                (self, next),
                (
                    Self::Pending,
                    Self::Applied | Self::Skipped | Self::Compensated | Self::Unknown
                ) | (Self::Applied, Self::Compensated | Self::Unknown)
                    | (Self::Skipped, Self::Unknown)
                    | (Self::Unknown, Self::Compensated)
            )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernedDataImportOwnerPlan {
    pub owner: String,
    pub import_target: String,
    pub before_digest: String,
    pub target_digest: String,
    pub item_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernedDataImportPrepare {
    pub operation_id: String,
    pub payload_digest: String,
    pub request_digest: String,
    pub owners: Vec<GovernedDataImportOwnerPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernedDataImportPrepareOutcome {
    pub receipt: GovernedDataImportReceipt,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernedDataImportOwnerUpdate {
    pub owner: String,
    pub status: GovernedDataImportOwnerStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernedDataImportOwnerReceipt {
    pub owner: String,
    pub import_target: String,
    pub before_digest: String,
    pub target_digest: String,
    pub item_count: u64,
    pub status: GovernedDataImportOwnerStatus,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernedDataImportOwnerObservation {
    pub owner: String,
    pub observed_digest: String,
    pub observed_at: DateTime<Utc>,
    pub state_restore_request_digest: Option<String>,
    pub state_restore_payload_digest: Option<String>,
    pub state_restore_before_canonical_digest: Option<String>,
    pub state_restore_after_canonical_digest: Option<String>,
    pub state_restore_outbox_event_id: Option<String>,
    pub state_projection_delivery_state: Option<ProjectionDeliveryState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernedDataImportOwnerResolution {
    pub owner: String,
    pub observed_digest: String,
    pub observed_at: DateTime<Utc>,
    pub classification: GovernedDataImportResolutionClassification,
    pub state_restore_request_digest: Option<String>,
    pub state_restore_payload_digest: Option<String>,
    pub state_restore_before_canonical_digest: Option<String>,
    pub state_restore_after_canonical_digest: Option<String>,
    pub state_restore_outbox_event_id: Option<String>,
    pub state_projection_delivery_state: Option<ProjectionDeliveryState>,
}

/// Metadata-only evidence used to terminalize an import whose original
/// payload is no longer available. It records owner digests and delivery
/// identity only; canonical bodies must remain in their canonical owners.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernedDataImportOwnerResolutionEvidence {
    #[serde(flatten)]
    pub resolution: GovernedDataImportOwnerResolution,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernedDataImportReceipt {
    pub operation_id: String,
    pub payload_digest: String,
    pub request_digest: String,
    pub stage: GovernedDataImportStage,
    pub target_count: u32,
    pub owners: Vec<GovernedDataImportOwnerReceipt>,
    pub resolution_evidence: Vec<GovernedDataImportOwnerResolutionEvidence>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub terminal_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernedDataImportTransition {
    pub sequence: u64,
    pub from_stage: Option<GovernedDataImportStage>,
    pub to_stage: GovernedDataImportStage,
    pub reason_digest: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernedDataImportDrift {
    pub operation_id: String,
    pub field: String,
}

impl std::fmt::Display for GovernedDataImportDrift {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "governed data-import operation {} metadata drift: {}",
            self.operation_id, self.field
        )
    }
}

impl std::error::Error for GovernedDataImportDrift {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernedDataImportInvalidTransition {
    pub operation_id: String,
    pub current: GovernedDataImportStage,
    pub requested: GovernedDataImportStage,
}

impl std::fmt::Display for GovernedDataImportInvalidTransition {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "illegal governed data-import transition for {}: {} -> {}",
            self.operation_id,
            self.current.as_str(),
            self.requested.as_str()
        )
    }
}

impl std::error::Error for GovernedDataImportInvalidTransition {}

/// SQLite cannot widen an existing `CHECK(stage IN (...))` in place. Rebuild
/// only the operation table, under an immediate transaction with foreign-key
/// enforcement temporarily disabled, then verify the complete graph before
/// the journal is admitted. Owner and transition rows remain untouched.
fn migrate_governed_data_import_abandonment_schema(conn: &mut Connection) -> Result<()> {
    let operation_schema: String = conn.query_row(
        "SELECT sql FROM sqlite_master
         WHERE type = 'table' AND name = 'governed_data_import_operations'",
        [],
        |row| row.get(0),
    )?;
    let needs_rebuild = !operation_schema.contains("'abandoned_preserving_current'");

    if needs_rebuild {
        conn.pragma_update(None, "foreign_keys", "OFF")?;
        let migration = (|| -> Result<()> {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(
                "DROP INDEX IF EXISTS idx_governed_data_import_one_unresolved;
                 DROP INDEX IF EXISTS idx_governed_data_import_stage;
                 CREATE TABLE governed_data_import_operations_v2 (
                    operation_id TEXT PRIMARY KEY,
                    payload_digest TEXT NOT NULL,
                    request_digest TEXT NOT NULL,
                    stage TEXT NOT NULL CHECK(stage IN (
                        'prepared', 'lifemodel_applied', 'memory_applied',
                        'vector_applied', 'state_committed', 'projection_degraded',
                        'completed', 'compensated', 'compensation_unknown',
                        'abandoned_preserving_current'
                    )),
                    target_count INTEGER NOT NULL CHECK(target_count > 0),
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    terminal_at TEXT
                 );
                 INSERT INTO governed_data_import_operations_v2 (
                    operation_id, payload_digest, request_digest, stage,
                    target_count, created_at, updated_at, terminal_at
                 )
                 SELECT operation_id, payload_digest, request_digest, stage,
                        target_count, created_at, updated_at, terminal_at
                 FROM governed_data_import_operations;
                 DROP TABLE governed_data_import_operations;
                 ALTER TABLE governed_data_import_operations_v2
                    RENAME TO governed_data_import_operations;",
            )?;
            tx.commit()?;
            Ok(())
        })();
        let foreign_keys_result = conn.pragma_update(None, "foreign_keys", "ON");
        migration?;
        foreign_keys_result?;
    }
    // Idempotent and intentionally outside `needs_rebuild`: a crash can occur
    // after the table rebuild commit but before the legacy graph rewrite. The
    // next bootstrap must still finish the exact-shape normalization.
    normalize_legacy_governed_import_compensation_unknown_roots(conn)?;

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch(
        "DROP INDEX IF EXISTS idx_governed_data_import_one_unresolved;
         CREATE UNIQUE INDEX idx_governed_data_import_one_unresolved
         ON governed_data_import_operations((1))
         WHERE stage NOT IN (
            'completed', 'compensated', 'abandoned_preserving_current'
         );
         CREATE INDEX IF NOT EXISTS idx_governed_data_import_stage
         ON governed_data_import_operations(stage, updated_at);",
    )?;
    tx.commit()?;
    ensure_governed_data_import_foreign_keys(conn)
}

/// The pre-abandonment schema could contain the original one-edge
/// `NULL -> compensation_unknown` recovery graph. Normalize only that exact,
/// unambiguous legacy shape to the current `NULL -> prepared ->
/// compensation_unknown` graph. Any more complex or disconnected historical
/// graph is intentionally left untouched and will fail closed if a caller
/// attempts terminalization.
fn normalize_legacy_governed_import_compensation_unknown_roots(
    conn: &mut Connection,
) -> Result<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let candidates = {
        let mut statement = tx.prepare(
            "SELECT operations.operation_id, operations.created_at
             FROM governed_data_import_operations operations
             JOIN governed_data_import_transitions transitions
               ON transitions.operation_id = operations.operation_id
              AND transitions.sequence = 0
             WHERE operations.stage = 'compensation_unknown'
               AND transitions.from_stage IS NULL
               AND transitions.to_stage = 'compensation_unknown'
               AND (
                    SELECT COUNT(*) FROM governed_data_import_transitions all_transitions
                    WHERE all_transitions.operation_id = operations.operation_id
               ) = 1
             ORDER BY operations.operation_id ASC",
        )?;
        let candidates = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        candidates
    };
    if candidates.is_empty() {
        tx.commit()?;
        return Ok(());
    }
    for (operation_id, created_at) in candidates {
        let changed = tx.execute(
            "UPDATE governed_data_import_transitions
             SET sequence = 1, from_stage = 'prepared'
             WHERE operation_id = ?1 AND sequence = 0
               AND from_stage IS NULL AND to_stage = 'compensation_unknown'",
            [&operation_id],
        )?;
        if changed != 1 {
            anyhow::bail!(
                "legacy governed data-import transition root changed during normalization"
            );
        }
        tx.execute(
            "INSERT INTO governed_data_import_transitions (
                operation_id, sequence, from_stage, to_stage,
                reason_digest, created_at
             ) VALUES (?1, 0, NULL, 'prepared', NULL, ?2)",
            params![operation_id, created_at],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn ensure_governed_data_import_foreign_keys(conn: &Connection) -> Result<()> {
    let enabled: i64 = conn.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
    if enabled != 1 {
        anyhow::bail!("governed data-import journal foreign keys are disabled");
    }
    let violation: Option<String> = conn
        .query_row("PRAGMA foreign_key_check", [], |row| row.get(0))
        .optional()?;
    if let Some(table) = violation {
        anyhow::bail!("governed data-import foreign-key drift in table {table}");
    }
    Ok(())
}

fn migrate_governed_data_import_resolution_evidence_schema(conn: &Connection) -> Result<()> {
    let mut statement =
        conn.prepare("PRAGMA table_info(governed_data_import_resolution_evidence)")?;
    let existing = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<std::collections::HashSet<_>, _>>()?;
    drop(statement);
    // `ADD COLUMN` preserves intermediate-schema rows and can carry the
    // projection enum constraint. SQLite cannot retrofit the new six-column
    // all-or-none table CHECK without rebuilding; receipt validation already
    // enforces that invariant and fails closed on partial terminal evidence.
    for (column, definition) in [
        ("state_restore_request_digest", "TEXT"),
        ("state_restore_payload_digest", "TEXT"),
        ("state_restore_before_canonical_digest", "TEXT"),
        ("state_restore_after_canonical_digest", "TEXT"),
        ("state_restore_outbox_event_id", "TEXT"),
        (
            "state_projection_delivery_state",
            "TEXT CHECK(
                state_projection_delivery_state IS NULL OR
                state_projection_delivery_state IN (
                    'pending', 'degraded', 'applied', 'superseded', 'compensated'
                )
            )",
        ),
    ] {
        if !existing.contains(column) {
            conn.execute(
                &format!(
                    "ALTER TABLE governed_data_import_resolution_evidence
                     ADD COLUMN {column} {definition}"
                ),
                [],
            )?;
        }
    }
    Ok(())
}

fn validate_governed_import_abandoned_terminal_graphs(conn: &Connection) -> Result<()> {
    let mut statement = conn.prepare(
        "SELECT operation_id FROM governed_data_import_operations
         WHERE stage = 'abandoned_preserving_current'
         ORDER BY operation_id ASC",
    )?;
    let operation_ids = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(statement);
    for operation_id in operation_ids {
        validate_governed_import_abandoned_terminal_graph(conn, &operation_id)?;
    }
    Ok(())
}

fn validate_governed_import_abandoned_terminal_graph(
    conn: &Connection,
    operation_id: &str,
) -> Result<()> {
    let receipt = governed_data_import_receipt_in(conn, operation_id)?
        .ok_or_else(|| anyhow::anyhow!("abandoned governed import disappeared"))?;
    if receipt.stage != GovernedDataImportStage::AbandonedPreservingCurrent
        || receipt.resolution_evidence.len() != receipt.owners.len()
    {
        anyhow::bail!("abandoned governed data-import terminal graph is incomplete");
    }
    let mut transitions_statement = conn.prepare(
        "SELECT sequence, from_stage, to_stage, reason_digest, created_at
         FROM governed_data_import_transitions
         WHERE operation_id = ?1
         ORDER BY sequence ASC",
    )?;
    let transitions = transitions_statement
        .query_map([operation_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if transitions.is_empty() {
        anyhow::bail!("abandoned governed data-import transition graph is empty");
    }
    let mut previous_to = None;
    let mut previous_created_at = None;
    for (index, (sequence, from_raw, to_raw, reason_digest, created_at_raw)) in
        transitions.iter().enumerate()
    {
        if *sequence != i64::try_from(index)? {
            anyhow::bail!("abandoned governed data-import transition sequence drift");
        }
        let to = GovernedDataImportStage::parse(to_raw)?;
        let from = from_raw
            .as_deref()
            .map(GovernedDataImportStage::parse)
            .transpose()?;
        if index == 0 {
            if from.is_some() || to != GovernedDataImportStage::Prepared || reason_digest.is_some()
            {
                anyhow::bail!("abandoned governed data-import transition root is invalid");
            }
        } else {
            let expected_from = previous_to.ok_or_else(|| {
                anyhow::anyhow!("abandoned governed data-import transition predecessor missing")
            })?;
            if from != Some(expected_from) {
                anyhow::bail!("abandoned governed data-import transition edge is disconnected");
            }
            let valid_edge = if to == GovernedDataImportStage::AbandonedPreservingCurrent {
                !expected_from.is_terminal()
            } else {
                to != expected_from && expected_from.may_transition_to(to)
            };
            if !valid_edge || expected_from.is_terminal() {
                anyhow::bail!("abandoned governed data-import transition edge is illegal");
            }
        }
        if let Some(reason_digest) = reason_digest.as_deref() {
            validate_digest(reason_digest)?;
        }
        let created_at = DateTime::parse_from_rfc3339(created_at_raw)?.with_timezone(&Utc);
        if index == 0 && created_at != receipt.created_at {
            anyhow::bail!("abandoned governed data-import transition root timestamp drift");
        }
        if previous_created_at.is_some_and(|previous| created_at < previous) {
            anyhow::bail!("abandoned governed data-import transition time regressed");
        }
        previous_created_at = Some(created_at);
        previous_to = Some(to);
    }
    let (_, last_from, last_to, last_reason, last_created_at) =
        transitions.last().expect("checked non-empty");
    if last_to != GovernedDataImportStage::AbandonedPreservingCurrent.as_str()
        || last_from.is_none()
    {
        anyhow::bail!("abandoned governed data-import terminal transition is missing");
    }
    validate_digest(
        last_reason
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("abandoned import reason digest missing"))?,
    )?;
    let last_created_at = DateTime::parse_from_rfc3339(last_created_at)?.with_timezone(&Utc);
    if receipt.updated_at != last_created_at
        || receipt.terminal_at != Some(last_created_at)
        || previous_to != Some(receipt.stage)
    {
        anyhow::bail!("abandoned governed data-import terminal timestamps drift");
    }
    Ok(())
}

impl GovernedDataImportJournal {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "create governed data-import journal parent {}",
                    parent.display()
                )
            })?;
        }
        let mut conn = Connection::open(path)
            .with_context(|| format!("open governed data-import journal {}", path.display()))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS governed_data_import_operations (
                operation_id TEXT PRIMARY KEY,
                payload_digest TEXT NOT NULL,
                request_digest TEXT NOT NULL,
                stage TEXT NOT NULL CHECK(stage IN (
                    'prepared', 'lifemodel_applied', 'memory_applied',
                    'vector_applied', 'state_committed', 'projection_degraded',
                    'completed', 'compensated', 'compensation_unknown',
                    'abandoned_preserving_current'
                )),
                target_count INTEGER NOT NULL CHECK(target_count > 0),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                terminal_at TEXT
             );
             CREATE UNIQUE INDEX IF NOT EXISTS idx_governed_data_import_one_unresolved
             ON governed_data_import_operations((1))
             WHERE stage NOT IN (
                'completed', 'compensated', 'abandoned_preserving_current'
             );
             CREATE INDEX IF NOT EXISTS idx_governed_data_import_stage
             ON governed_data_import_operations(stage, updated_at);
             CREATE TABLE IF NOT EXISTS governed_data_import_owners (
                operation_id TEXT NOT NULL,
                owner TEXT NOT NULL,
                import_target TEXT NOT NULL,
                before_digest TEXT NOT NULL,
                target_digest TEXT NOT NULL,
                item_count INTEGER NOT NULL CHECK(item_count >= 0),
                status TEXT NOT NULL CHECK(status IN (
                    'pending', 'applied', 'skipped', 'compensated', 'unknown'
                )),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY(operation_id, owner),
                UNIQUE(operation_id, import_target),
                FOREIGN KEY(operation_id)
                    REFERENCES governed_data_import_operations(operation_id)
                    ON DELETE CASCADE
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS governed_data_import_transitions (
                operation_id TEXT NOT NULL,
                sequence INTEGER NOT NULL CHECK(sequence >= 0),
                from_stage TEXT,
                to_stage TEXT NOT NULL,
                reason_digest TEXT,
                created_at TEXT NOT NULL,
                PRIMARY KEY(operation_id, sequence),
                FOREIGN KEY(operation_id)
                    REFERENCES governed_data_import_operations(operation_id)
                    ON DELETE CASCADE
             ) WITHOUT ROWID;",
        )?;
        migrate_governed_data_import_abandonment_schema(&mut conn)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS governed_data_import_resolution_evidence (
                operation_id TEXT NOT NULL,
                owner TEXT NOT NULL,
                observed_digest TEXT NOT NULL,
                observed_at TEXT NOT NULL,
                classification TEXT NOT NULL CHECK(classification IN (
                    'before', 'target', 'other'
                )),
                state_restore_request_digest TEXT,
                state_restore_payload_digest TEXT,
                state_restore_before_canonical_digest TEXT,
                state_restore_after_canonical_digest TEXT,
                state_restore_outbox_event_id TEXT,
                state_projection_delivery_state TEXT CHECK(
                    state_projection_delivery_state IS NULL OR
                    state_projection_delivery_state IN (
                        'pending', 'degraded', 'applied', 'superseded', 'compensated'
                    )
                ),
                recorded_at TEXT NOT NULL,
                PRIMARY KEY(operation_id, owner),
                CHECK(
                    (state_restore_request_digest IS NULL AND
                     state_restore_payload_digest IS NULL AND
                     state_restore_before_canonical_digest IS NULL AND
                     state_restore_after_canonical_digest IS NULL AND
                     state_restore_outbox_event_id IS NULL AND
                     state_projection_delivery_state IS NULL) OR
                    (state_restore_request_digest IS NOT NULL AND
                     state_restore_payload_digest IS NOT NULL AND
                     state_restore_before_canonical_digest IS NOT NULL AND
                     state_restore_after_canonical_digest IS NOT NULL AND
                     state_restore_outbox_event_id IS NOT NULL AND
                     state_projection_delivery_state IS NOT NULL)
                ),
                FOREIGN KEY(operation_id, owner)
                    REFERENCES governed_data_import_owners(operation_id, owner)
                    ON DELETE CASCADE
             ) WITHOUT ROWID;",
        )?;
        migrate_governed_data_import_resolution_evidence_schema(&conn)?;
        ensure_governed_data_import_foreign_keys(&conn)?;
        validate_governed_import_abandoned_terminal_graphs(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// A `prepared` receipt alone cannot prove
    /// whether this call created the saga or reopened an interrupted one;
    /// callers must inspect `replayed` and route replays to reconciliation
    /// instead of blindly reapplying owner effects.
    pub fn prepare(
        &self,
        mut input: GovernedDataImportPrepare,
    ) -> Result<GovernedDataImportPrepareOutcome> {
        validate_governed_import_operation_id(&input.operation_id)?;
        validate_digest(&input.payload_digest)?;
        validate_digest(&input.request_digest)?;
        validate_governed_import_owners(&input.owners)?;
        input
            .owners
            .sort_by(|left, right| left.owner.cmp(&right.owner));

        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = governed_data_import_receipt_in(&tx, &input.operation_id)? {
            ensure_governed_import_prepare_replay_matches(&existing, &input)?;
            tx.commit()?;
            return Ok(GovernedDataImportPrepareOutcome {
                receipt: existing,
                replayed: true,
            });
        }
        let unresolved: Option<String> = tx
            .query_row(
                "SELECT operation_id FROM governed_data_import_operations
                 WHERE stage NOT IN (
                    'completed', 'compensated', 'abandoned_preserving_current'
                 )
                 ORDER BY created_at ASC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(operation_id) = unresolved {
            anyhow::bail!(
                "governed data-import prepare blocked by unresolved operation {operation_id}"
            );
        }
        let now = Utc::now();
        let now_text = now.to_rfc3339();
        tx.execute(
            "INSERT INTO governed_data_import_operations (
                operation_id, payload_digest, request_digest, stage,
                target_count, created_at, updated_at, terminal_at
             ) VALUES (?1, ?2, ?3, 'prepared', ?4, ?5, ?5, NULL)",
            params![
                input.operation_id,
                input.payload_digest,
                input.request_digest,
                i64::try_from(input.owners.len())?,
                now_text,
            ],
        )?;
        for owner in &input.owners {
            tx.execute(
                "INSERT INTO governed_data_import_owners (
                    operation_id, owner, import_target, before_digest,
                    target_digest, item_count, status, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7, ?7)",
                params![
                    input.operation_id,
                    owner.owner,
                    owner.import_target,
                    owner.before_digest,
                    owner.target_digest,
                    i64::try_from(owner.item_count)?,
                    now_text,
                ],
            )?;
        }
        tx.execute(
            "INSERT INTO governed_data_import_transitions (
                operation_id, sequence, from_stage, to_stage,
                reason_digest, created_at
             ) VALUES (?1, 0, NULL, 'prepared', NULL, ?2)",
            params![input.operation_id, now_text],
        )?;
        let receipt = governed_data_import_receipt_in(&tx, &input.operation_id)?
            .ok_or_else(|| anyhow::anyhow!("prepared governed data-import receipt disappeared"))?;
        tx.commit()?;
        Ok(GovernedDataImportPrepareOutcome {
            receipt,
            replayed: false,
        })
    }

    pub fn transition(
        &self,
        operation_id: &str,
        next_stage: GovernedDataImportStage,
        owner_updates: &[GovernedDataImportOwnerUpdate],
        reason_digest: Option<&str>,
    ) -> Result<GovernedDataImportReceipt> {
        validate_governed_import_operation_id(operation_id)?;
        if next_stage == GovernedDataImportStage::AbandonedPreservingCurrent {
            anyhow::bail!("abandoned_preserving_current requires atomic owner-resolution evidence");
        }
        if let Some(digest) = reason_digest {
            validate_digest(digest)?;
        }
        validate_governed_import_owner_updates(owner_updates)?;
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current_receipt = governed_data_import_receipt_in(&tx, operation_id)?
            .ok_or_else(|| anyhow::anyhow!("governed data-import operation missing"))?;
        if !current_receipt.stage.may_transition_to(next_stage) {
            return Err(GovernedDataImportInvalidTransition {
                operation_id: operation_id.to_string(),
                current: current_receipt.stage,
                requested: next_stage,
            }
            .into());
        }

        if current_receipt.stage == next_stage {
            ensure_governed_import_transition_replay_matches(&current_receipt, owner_updates)?;
            let stored_reason_digest: Option<String> = tx.query_row(
                "SELECT reason_digest FROM governed_data_import_transitions
                 WHERE operation_id = ?1 ORDER BY sequence DESC LIMIT 1",
                [operation_id],
                |row| row.get(0),
            )?;
            if stored_reason_digest.as_deref() != reason_digest {
                return Err(GovernedDataImportDrift {
                    operation_id: operation_id.to_string(),
                    field: "transition_reason_digest".into(),
                }
                .into());
            }
            tx.commit()?;
            return Ok(current_receipt);
        }

        let now = Utc::now();
        let now_text = now.to_rfc3339();
        for update in owner_updates {
            let current = current_receipt
                .owners
                .iter()
                .find(|owner| owner.owner == update.owner)
                .ok_or_else(|| GovernedDataImportDrift {
                    operation_id: operation_id.to_string(),
                    field: format!("unknown owner {}", update.owner),
                })?;
            if !current.status.may_transition_to(update.status) {
                return Err(GovernedDataImportDrift {
                    operation_id: operation_id.to_string(),
                    field: format!("owner {} status", update.owner),
                }
                .into());
            }
            tx.execute(
                "UPDATE governed_data_import_owners
                 SET status = ?3, updated_at = ?4
                 WHERE operation_id = ?1 AND owner = ?2",
                params![operation_id, update.owner, update.status.as_str(), now_text],
            )?;
        }

        validate_governed_import_stage_facts(&tx, operation_id, next_stage, reason_digest)?;
        let terminal_at = next_stage.is_terminal().then_some(now_text.as_str());
        tx.execute(
            "UPDATE governed_data_import_operations
             SET stage = ?2, updated_at = ?3, terminal_at = ?4
             WHERE operation_id = ?1",
            params![operation_id, next_stage.as_str(), now_text, terminal_at],
        )?;
        let sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence), -1) + 1
             FROM governed_data_import_transitions WHERE operation_id = ?1",
            [operation_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO governed_data_import_transitions (
                operation_id, sequence, from_stage, to_stage,
                reason_digest, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                operation_id,
                sequence,
                current_receipt.stage.as_str(),
                next_stage.as_str(),
                reason_digest,
                now_text,
            ],
        )?;
        let receipt = governed_data_import_receipt_in(&tx, operation_id)?
            .ok_or_else(|| anyhow::anyhow!("governed data-import receipt disappeared"))?;
        tx.commit()?;
        Ok(receipt)
    }

    /// Terminalize an unresolved import without its original payload. This is
    /// intentionally payload-independent: every owner is re-observed, the
    /// digest is classified against the durable before/target plan, and all
    /// evidence plus the terminal transition are committed atomically.
    /// Historical owner statuses are not rewritten.
    pub fn preview_abandonment_resolutions(
        &self,
        operation_id: &str,
        observations: &[GovernedDataImportOwnerObservation],
    ) -> Result<Vec<GovernedDataImportOwnerResolution>> {
        validate_governed_import_operation_id(operation_id)?;
        validate_governed_import_observations(observations)?;
        let conn = self.lock()?;
        let receipt = governed_data_import_receipt_in(&conn, operation_id)?
            .ok_or_else(|| anyhow::anyhow!("governed data-import operation missing"))?;
        derive_governed_import_resolution_facts(
            operation_id,
            &receipt.request_digest,
            &receipt.owners,
            observations,
            governed_import_state_commit_was_recorded(&conn, operation_id)?,
        )
    }

    pub fn abandon_preserving_current(
        &self,
        operation_id: &str,
        observations: &[GovernedDataImportOwnerObservation],
        reason_digest: &str,
    ) -> Result<GovernedDataImportReceipt> {
        validate_governed_import_operation_id(operation_id)?;
        validate_digest(reason_digest)?;
        validate_governed_import_observations(observations)?;

        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current_receipt = governed_data_import_receipt_in(&tx, operation_id)?
            .ok_or_else(|| anyhow::anyhow!("governed data-import operation missing"))?;

        if current_receipt.stage == GovernedDataImportStage::AbandonedPreservingCurrent {
            ensure_governed_import_resolution_replay_matches(
                &current_receipt,
                observations,
                reason_digest,
                &tx,
            )?;
            tx.commit()?;
            return Ok(current_receipt);
        }
        if current_receipt.stage.is_terminal() {
            return Err(GovernedDataImportInvalidTransition {
                operation_id: operation_id.to_string(),
                current: current_receipt.stage,
                requested: GovernedDataImportStage::AbandonedPreservingCurrent,
            }
            .into());
        }

        let ordered = derive_governed_import_resolution_facts(
            operation_id,
            &current_receipt.request_digest,
            &current_receipt.owners,
            observations,
            governed_import_state_commit_was_recorded(&tx, operation_id)?,
        )?;
        let now = Utc::now();
        let now_text = now.to_rfc3339();
        for resolution in ordered {
            tx.execute(
                "INSERT INTO governed_data_import_resolution_evidence (
                    operation_id, owner, observed_digest, observed_at,
                    classification, state_restore_request_digest,
                    state_restore_payload_digest,
                    state_restore_before_canonical_digest,
                    state_restore_after_canonical_digest,
                    state_restore_outbox_event_id,
                    state_projection_delivery_state, recorded_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    operation_id,
                    resolution.owner,
                    resolution.observed_digest,
                    resolution.observed_at.to_rfc3339(),
                    resolution.classification.as_str(),
                    resolution.state_restore_request_digest,
                    resolution.state_restore_payload_digest,
                    resolution.state_restore_before_canonical_digest,
                    resolution.state_restore_after_canonical_digest,
                    resolution.state_restore_outbox_event_id,
                    resolution
                        .state_projection_delivery_state
                        .map(ProjectionDeliveryState::as_str),
                    now_text,
                ],
            )?;
        }

        tx.execute(
            "UPDATE governed_data_import_operations
             SET stage = 'abandoned_preserving_current', updated_at = ?2,
                 terminal_at = ?2
             WHERE operation_id = ?1",
            params![operation_id, now_text],
        )?;
        let sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence), -1) + 1
             FROM governed_data_import_transitions WHERE operation_id = ?1",
            [operation_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO governed_data_import_transitions (
                operation_id, sequence, from_stage, to_stage,
                reason_digest, created_at
             ) VALUES (?1, ?2, ?3, 'abandoned_preserving_current', ?4, ?5)",
            params![
                operation_id,
                sequence,
                current_receipt.stage.as_str(),
                reason_digest,
                now_text,
            ],
        )?;
        let receipt = governed_data_import_receipt_in(&tx, operation_id)?
            .ok_or_else(|| anyhow::anyhow!("abandoned data-import receipt disappeared"))?;
        // Validate exactly the graph that will be admitted after restart while
        // it is still inside the same transaction. A malformed historical or
        // concurrently corrupted graph must roll back the evidence and
        // terminal row instead of reporting success that the next bootstrap
        // cannot reopen.
        validate_governed_import_abandoned_terminal_graph(&tx, operation_id)?;
        tx.commit()?;
        Ok(receipt)
    }

    pub fn receipt(&self, operation_id: &str) -> Result<Option<GovernedDataImportReceipt>> {
        validate_governed_import_operation_id(operation_id)?;
        let conn = self.lock()?;
        governed_data_import_receipt_in(&conn, operation_id)
    }

    pub fn terminal_receipt(
        &self,
        operation_id: &str,
    ) -> Result<Option<GovernedDataImportReceipt>> {
        Ok(self
            .receipt(operation_id)?
            .filter(|receipt| receipt.stage.is_terminal()))
    }

    /// Latest durable import disposition for bounded status/read models. The
    /// receipt contains only digests, counts, owner metadata, and timestamps;
    /// original import bodies remain in their canonical owners/caller input.
    pub fn latest_receipt(&self) -> Result<Option<GovernedDataImportReceipt>> {
        let conn = self.lock()?;
        let operation_id: Option<String> = conn
            .query_row(
                "SELECT operation_id FROM governed_data_import_operations
                 ORDER BY updated_at DESC, created_at DESC, operation_id DESC
                 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        match operation_id {
            Some(operation_id) => governed_data_import_receipt_in(&conn, &operation_id),
            None => Ok(None),
        }
    }

    pub fn recovery_requirement(&self) -> Result<Option<GovernedDataImportReceipt>> {
        let conn = self.lock()?;
        let operation_id: Option<String> = conn
            .query_row(
                "SELECT operation_id FROM governed_data_import_operations
                 WHERE stage NOT IN (
                    'completed', 'compensated', 'abandoned_preserving_current'
                 )
                 ORDER BY created_at ASC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        match operation_id {
            Some(operation_id) => governed_data_import_receipt_in(&conn, &operation_id),
            None => Ok(None),
        }
    }

    pub fn transitions(&self, operation_id: &str) -> Result<Vec<GovernedDataImportTransition>> {
        validate_governed_import_operation_id(operation_id)?;
        let conn = self.lock()?;
        let mut statement = conn.prepare(
            "SELECT sequence, from_stage, to_stage, reason_digest, created_at
             FROM governed_data_import_transitions
             WHERE operation_id = ?1 ORDER BY sequence ASC",
        )?;
        let rows = statement.query_map([operation_id], |row| {
            let from_stage = row
                .get::<_, Option<String>>(1)?
                .map(|value| parse_governed_import_stage_sql(1, &value))
                .transpose()?;
            let to_stage_raw: String = row.get(2)?;
            Ok(GovernedDataImportTransition {
                sequence: u64::try_from(row.get::<_, i64>(0)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                from_stage,
                to_stage: parse_governed_import_stage_sql(2, &to_stage_raw)?,
                reason_digest: row.get(3)?,
                created_at: parse_timestamp(row, 4)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|error| anyhow::anyhow!("governed data-import journal mutex poison: {error}"))
    }
}

fn validate_governed_import_operation_id(operation_id: &str) -> Result<()> {
    let parsed = Uuid::parse_str(operation_id)
        .map_err(|_| anyhow::anyhow!("governed data-import operation_id must be UUIDv4"))?;
    if parsed.get_version() != Some(uuid::Version::Random)
        || parsed.hyphenated().to_string() != operation_id
    {
        anyhow::bail!("governed data-import operation_id must be canonical UUIDv4");
    }
    Ok(())
}

fn validate_governed_import_owners(owners: &[GovernedDataImportOwnerPlan]) -> Result<()> {
    if owners.is_empty() || owners.len() > MAX_GOVERNED_IMPORT_OWNERS {
        anyhow::bail!("governed data-import owner count is out of bounds");
    }
    let mut unique_owners = std::collections::HashSet::new();
    let mut unique_targets = std::collections::HashSet::new();
    for owner in owners {
        validate_identifier("data_import_owner", &owner.owner)?;
        validate_identifier("data_import_target", &owner.import_target)?;
        if !matches!(
            (owner.owner.as_str(), owner.import_target.as_str()),
            ("LifeModelFileStore", "life_model")
                | ("MemoryStore", "messages")
                | ("VectorStore", "vectors")
                | ("StateStore", "state_store")
        ) {
            anyhow::bail!(
                "unsupported governed data-import owner/target mapping: {}/{}",
                owner.owner,
                owner.import_target,
            );
        }
        validate_digest(&owner.before_digest)?;
        validate_digest(&owner.target_digest)?;
        i64::try_from(owner.item_count)
            .map_err(|_| anyhow::anyhow!("governed data-import item_count exceeds SQLite"))?;
        if !unique_owners.insert(owner.owner.as_str()) {
            anyhow::bail!("duplicate governed data-import owner: {}", owner.owner);
        }
        if !unique_targets.insert(owner.import_target.as_str()) {
            anyhow::bail!(
                "duplicate governed data-import target: {}",
                owner.import_target
            );
        }
    }
    if !owners
        .iter()
        .any(|owner| owner.import_target == "life_model")
    {
        anyhow::bail!("governed data-import requires a LifeModel owner plan");
    }
    Ok(())
}

fn validate_governed_import_owner_updates(updates: &[GovernedDataImportOwnerUpdate]) -> Result<()> {
    let mut unique = std::collections::HashSet::new();
    for update in updates {
        validate_identifier("data_import_owner", &update.owner)?;
        if !unique.insert(update.owner.as_str()) {
            anyhow::bail!(
                "duplicate governed data-import owner update: {}",
                update.owner
            );
        }
    }
    Ok(())
}

fn validate_governed_import_observations(
    observations: &[GovernedDataImportOwnerObservation],
) -> Result<()> {
    if observations.is_empty() || observations.len() > MAX_GOVERNED_IMPORT_OWNERS {
        anyhow::bail!("governed data-import resolution count is out of bounds");
    }
    let mut unique = std::collections::HashSet::new();
    for observation in observations {
        validate_identifier("data_import_resolution_owner", &observation.owner)?;
        validate_digest(&observation.observed_digest)?;
        if !unique.insert(observation.owner.as_str()) {
            anyhow::bail!(
                "duplicate governed data-import resolution owner: {}",
                observation.owner
            );
        }
        let proof_presence = [
            observation.state_restore_request_digest.is_some(),
            observation.state_restore_payload_digest.is_some(),
            observation.state_restore_before_canonical_digest.is_some(),
            observation.state_restore_after_canonical_digest.is_some(),
            observation.state_restore_outbox_event_id.is_some(),
            observation.state_projection_delivery_state.is_some(),
        ];
        if proof_presence.iter().any(|present| *present)
            && !proof_presence.iter().all(|present| *present)
        {
            anyhow::bail!("governed data-import StateStore resolution evidence must be complete");
        }
        if proof_presence.iter().all(|present| *present) {
            for digest in [
                observation.state_restore_request_digest.as_deref(),
                observation.state_restore_payload_digest.as_deref(),
                observation.state_restore_before_canonical_digest.as_deref(),
                observation.state_restore_after_canonical_digest.as_deref(),
            ] {
                validate_digest(digest.expect("complete StateStore proof"))?;
            }
            if let Some(event_id) = observation.state_restore_outbox_event_id.as_deref() {
                validate_identifier("state_restore_outbox_event_id", event_id)?;
                let parsed = event_id
                    .strip_prefix("outbox:")
                    .and_then(|value| Uuid::parse_str(value).ok());
                if !parsed.is_some_and(|value| {
                    value.get_version() == Some(uuid::Version::Random)
                        && format!("outbox:{}", value.hyphenated()) == event_id
                }) {
                    anyhow::bail!(
                        "governed data-import StateStore resolution requires an exact outbox event id"
                    );
                }
            }
        }
    }
    Ok(())
}

fn governed_import_resolution_classification(
    owner: &GovernedDataImportOwnerReceipt,
    observed_digest: &str,
) -> GovernedDataImportResolutionClassification {
    // Target takes precedence when before and target are identical. This makes
    // the persisted category deterministic for no-op owner plans.
    if observed_digest == owner.target_digest {
        GovernedDataImportResolutionClassification::Target
    } else if observed_digest == owner.before_digest {
        GovernedDataImportResolutionClassification::Before
    } else {
        GovernedDataImportResolutionClassification::Other
    }
}

fn governed_import_state_commit_was_recorded(
    conn: &Connection,
    operation_id: &str,
) -> Result<bool> {
    let recorded: i64 = conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM governed_data_import_transitions
            WHERE operation_id = ?1
              AND to_stage IN ('state_committed', 'projection_degraded')
         )",
        [operation_id],
        |row| row.get(0),
    )?;
    Ok(recorded != 0)
}

fn derive_governed_import_resolution_facts(
    operation_id: &str,
    request_digest: &str,
    owners: &[GovernedDataImportOwnerReceipt],
    observations: &[GovernedDataImportOwnerObservation],
    state_commit_was_recorded: bool,
) -> Result<Vec<GovernedDataImportOwnerResolution>> {
    if owners.len() != observations.len() {
        return Err(GovernedDataImportDrift {
            operation_id: operation_id.to_string(),
            field: "resolution_owner_coverage".into(),
        }
        .into());
    }
    let mut ordered = observations.to_vec();
    ordered.sort_by(|left, right| left.owner.cmp(&right.owner));
    let mut resolutions = Vec::with_capacity(ordered.len());
    for (owner, observation) in owners.iter().zip(ordered) {
        if owner.owner != observation.owner {
            return Err(GovernedDataImportDrift {
                operation_id: operation_id.to_string(),
                field: "resolution_owner_coverage".into(),
            }
            .into());
        }
        let is_state_store = owner.import_target == "state_store";
        let has_state_proof = observation.state_restore_outbox_event_id.is_some();
        if !is_state_store && has_state_proof {
            return Err(GovernedDataImportDrift {
                operation_id: operation_id.to_string(),
                field: format!("owner {} StateStore resolution proof", owner.owner),
            }
            .into());
        }
        let classification = if is_state_store && has_state_proof {
            if observation.state_restore_request_digest.as_deref() != Some(request_digest)
                || observation.state_restore_payload_digest.as_deref()
                    != Some(owner.target_digest.as_str())
                || observation.state_restore_before_canonical_digest.as_deref()
                    != Some(owner.before_digest.as_str())
            {
                return Err(GovernedDataImportDrift {
                    operation_id: operation_id.to_string(),
                    field: format!("owner {} StateStore restore binding", owner.owner),
                }
                .into());
            }
            if observation.state_restore_after_canonical_digest.as_deref()
                == Some(observation.observed_digest.as_str())
            {
                GovernedDataImportResolutionClassification::Target
            } else if observation.observed_digest == owner.before_digest {
                GovernedDataImportResolutionClassification::Before
            } else {
                GovernedDataImportResolutionClassification::Other
            }
        } else if is_state_store {
            if state_commit_was_recorded {
                return Err(GovernedDataImportDrift {
                    operation_id: operation_id.to_string(),
                    field: format!("owner {} missing committed StateStore proof", owner.owner),
                }
                .into());
            }
            if observation.observed_digest == owner.before_digest {
                GovernedDataImportResolutionClassification::Before
            } else {
                // Without an exact restore receipt, a StateStore digest can be
                // attributed only to before or other, never to the portable
                // payload target (a different digest domain).
                GovernedDataImportResolutionClassification::Other
            }
        } else {
            governed_import_resolution_classification(owner, &observation.observed_digest)
        };
        resolutions.push(GovernedDataImportOwnerResolution {
            owner: observation.owner,
            observed_digest: observation.observed_digest,
            observed_at: observation.observed_at,
            classification,
            state_restore_request_digest: observation.state_restore_request_digest,
            state_restore_payload_digest: observation.state_restore_payload_digest,
            state_restore_before_canonical_digest: observation
                .state_restore_before_canonical_digest,
            state_restore_after_canonical_digest: observation.state_restore_after_canonical_digest,
            state_restore_outbox_event_id: observation.state_restore_outbox_event_id,
            state_projection_delivery_state: observation.state_projection_delivery_state,
        });
    }
    Ok(resolutions)
}

fn ensure_governed_import_resolution_replay_matches(
    receipt: &GovernedDataImportReceipt,
    observations: &[GovernedDataImportOwnerObservation],
    reason_digest: &str,
    conn: &Connection,
) -> Result<()> {
    let ordered = derive_governed_import_resolution_facts(
        &receipt.operation_id,
        &receipt.request_digest,
        &receipt.owners,
        observations,
        governed_import_state_commit_was_recorded(conn, &receipt.operation_id)?,
    )?;
    let stored = receipt
        .resolution_evidence
        .iter()
        .map(|evidence| evidence.resolution.clone())
        .collect::<Vec<_>>();
    if stored != ordered {
        return Err(GovernedDataImportDrift {
            operation_id: receipt.operation_id.clone(),
            field: "resolution_evidence_replay".into(),
        }
        .into());
    }
    let (stage, stored_reason): (String, Option<String>) = conn.query_row(
        "SELECT to_stage, reason_digest FROM governed_data_import_transitions
         WHERE operation_id = ?1 ORDER BY sequence DESC LIMIT 1",
        [&receipt.operation_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if stage != GovernedDataImportStage::AbandonedPreservingCurrent.as_str()
        || stored_reason.as_deref() != Some(reason_digest)
    {
        return Err(GovernedDataImportDrift {
            operation_id: receipt.operation_id.clone(),
            field: "abandonment_reason_digest".into(),
        }
        .into());
    }
    Ok(())
}

fn ensure_governed_import_prepare_replay_matches(
    existing: &GovernedDataImportReceipt,
    input: &GovernedDataImportPrepare,
) -> Result<()> {
    let mismatch = if existing.payload_digest != input.payload_digest {
        Some("payload_digest")
    } else if existing.request_digest != input.request_digest {
        Some("request_digest")
    } else if existing.owners.len() != input.owners.len() {
        Some("target_count")
    } else {
        existing
            .owners
            .iter()
            .zip(&input.owners)
            .find_map(|(stored, requested)| {
                (stored.owner != requested.owner
                    || stored.import_target != requested.import_target
                    || stored.before_digest != requested.before_digest
                    || stored.target_digest != requested.target_digest
                    || stored.item_count != requested.item_count)
                    .then_some("owner_plan")
            })
    };
    if let Some(field) = mismatch {
        return Err(GovernedDataImportDrift {
            operation_id: input.operation_id.clone(),
            field: field.to_string(),
        }
        .into());
    }
    Ok(())
}

fn ensure_governed_import_transition_replay_matches(
    receipt: &GovernedDataImportReceipt,
    updates: &[GovernedDataImportOwnerUpdate],
) -> Result<()> {
    for update in updates {
        let matches = receipt
            .owners
            .iter()
            .any(|owner| owner.owner == update.owner && owner.status == update.status);
        if !matches {
            return Err(GovernedDataImportDrift {
                operation_id: receipt.operation_id.clone(),
                field: format!("owner {} transition replay", update.owner),
            }
            .into());
        }
    }
    Ok(())
}

fn validate_governed_import_stage_facts(
    conn: &Connection,
    operation_id: &str,
    stage: GovernedDataImportStage,
    reason_digest: Option<&str>,
) -> Result<()> {
    let status_for_target = |target: &str| -> Result<Option<GovernedDataImportOwnerStatus>> {
        let raw: Option<String> = conn
            .query_row(
                "SELECT status FROM governed_data_import_owners
                 WHERE operation_id = ?1 AND import_target = ?2",
                params![operation_id, target],
                |row| row.get(0),
            )
            .optional()?;
        raw.map(|value| GovernedDataImportOwnerStatus::parse(&value))
            .transpose()
    };
    let require_applied_if_present = |target: &str| -> Result<()> {
        if let Some(status) = status_for_target(target)? {
            if !matches!(
                status,
                GovernedDataImportOwnerStatus::Applied | GovernedDataImportOwnerStatus::Skipped
            ) {
                anyhow::bail!(
                    "governed data-import stage {} requires target {target} to be applied",
                    stage.as_str()
                );
            }
        }
        Ok(())
    };
    let require_lifemodel_applied = || -> Result<()> {
        match status_for_target("life_model")? {
            Some(
                GovernedDataImportOwnerStatus::Applied | GovernedDataImportOwnerStatus::Skipped,
            ) => Ok(()),
            _ => anyhow::bail!(
                "governed data-import stage {} requires target life_model to be applied",
                stage.as_str()
            ),
        }
    };
    match stage {
        GovernedDataImportStage::LifeModelApplied => require_lifemodel_applied()?,
        GovernedDataImportStage::MemoryApplied => {
            require_lifemodel_applied()?;
            require_applied_if_present("messages")?;
        }
        GovernedDataImportStage::VectorApplied => {
            require_lifemodel_applied()?;
            require_applied_if_present("messages")?;
            require_applied_if_present("vectors")?;
        }
        GovernedDataImportStage::StateCommitted => {
            require_lifemodel_applied()?;
            require_applied_if_present("messages")?;
            require_applied_if_present("vectors")?;
            require_applied_if_present("state_store")?;
        }
        GovernedDataImportStage::ProjectionDegraded => {
            let unfinished: i64 = conn.query_row(
                "SELECT COUNT(*) FROM governed_data_import_owners
                 WHERE operation_id = ?1 AND status NOT IN ('applied', 'skipped')",
                [operation_id],
                |row| row.get(0),
            )?;
            if unfinished != 0 {
                anyhow::bail!("projection_degraded has unfinished canonical owner status");
            }
            if reason_digest.is_none() {
                anyhow::bail!("projection_degraded requires a metadata-only reason digest");
            }
        }
        GovernedDataImportStage::Completed => {
            let remaining: i64 = conn.query_row(
                "SELECT COUNT(*) FROM governed_data_import_owners
                 WHERE operation_id = ?1 AND status NOT IN ('applied', 'skipped')",
                [operation_id],
                |row| row.get(0),
            )?;
            if remaining != 0 {
                anyhow::bail!("completed governed data-import has unfinished owner status");
            }
            if status_for_target("state_store")?.is_some() && reason_digest.is_none() {
                anyhow::bail!(
                    "completed governed data-import with StateStore target requires a projection evidence digest"
                );
            }
        }
        GovernedDataImportStage::Compensated => {
            let uncompensated: i64 = conn.query_row(
                "SELECT COUNT(*) FROM governed_data_import_owners
                 WHERE operation_id = ?1 AND status IN ('applied', 'unknown')",
                [operation_id],
                |row| row.get(0),
            )?;
            if uncompensated != 0 {
                anyhow::bail!(
                    "compensated governed data-import still has applied or unknown owner status"
                );
            }
        }
        GovernedDataImportStage::CompensationUnknown => {
            let unknown: i64 = conn.query_row(
                "SELECT COUNT(*) FROM governed_data_import_owners
                 WHERE operation_id = ?1 AND status = 'unknown'",
                [operation_id],
                |row| row.get(0),
            )?;
            if unknown == 0 || reason_digest.is_none() {
                anyhow::bail!(
                    "compensation_unknown requires an unknown owner and metadata-only reason digest"
                );
            }
        }
        GovernedDataImportStage::Prepared => {
            anyhow::bail!("prepared is created only by GovernedDataImportJournal::prepare")
        }
        GovernedDataImportStage::AbandonedPreservingCurrent => {
            anyhow::bail!("abandoned_preserving_current requires atomic owner-resolution evidence")
        }
    }
    Ok(())
}

fn governed_data_import_receipt_in(
    conn: &Connection,
    operation_id: &str,
) -> Result<Option<GovernedDataImportReceipt>> {
    type GovernedImportOperationRow = (String, String, String, i64, String, String, Option<String>);
    let operation: Option<GovernedImportOperationRow> = conn
        .query_row(
            "SELECT payload_digest, request_digest, stage, target_count,
                    created_at, updated_at, terminal_at
             FROM governed_data_import_operations WHERE operation_id = ?1",
            [operation_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .optional()?;
    let Some((
        payload_digest,
        request_digest,
        stage_raw,
        target_count,
        created_at,
        updated_at,
        terminal_at,
    )) = operation
    else {
        return Ok(None);
    };
    let mut statement = conn.prepare(
        "SELECT owner, import_target, before_digest, target_digest,
                item_count, status, updated_at
         FROM governed_data_import_owners
         WHERE operation_id = ?1 ORDER BY owner ASC",
    )?;
    let owners = statement
        .query_map([operation_id], |row| {
            let status_raw: String = row.get(5)?;
            Ok(GovernedDataImportOwnerReceipt {
                owner: row.get(0)?,
                import_target: row.get(1)?,
                before_digest: row.get(2)?,
                target_digest: row.get(3)?,
                item_count: u64::try_from(row.get::<_, i64>(4)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                status: parse_governed_import_owner_status_sql(5, &status_raw)?,
                updated_at: parse_timestamp(row, 6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut evidence_statement = conn.prepare(
        "SELECT owner, observed_digest, observed_at, classification,
                state_restore_request_digest,
                state_restore_payload_digest,
                state_restore_before_canonical_digest,
                state_restore_after_canonical_digest,
                state_restore_outbox_event_id,
                state_projection_delivery_state, recorded_at
         FROM governed_data_import_resolution_evidence
         WHERE operation_id = ?1 ORDER BY owner ASC",
    )?;
    let resolution_evidence = evidence_statement
        .query_map([operation_id], |row| {
            let classification_raw: String = row.get(3)?;
            let classification = GovernedDataImportResolutionClassification::parse(
                &classification_raw,
            )
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        error.to_string(),
                    )),
                )
            })?;
            let state_raw: Option<String> = row.get(9)?;
            let state_projection_delivery_state = state_raw
                .map(|value| {
                    ProjectionDeliveryState::parse(&value).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            9,
                            rusqlite::types::Type::Text,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                error.to_string(),
                            )),
                        )
                    })
                })
                .transpose()?;
            Ok(GovernedDataImportOwnerResolutionEvidence {
                resolution: GovernedDataImportOwnerResolution {
                    owner: row.get(0)?,
                    observed_digest: row.get(1)?,
                    observed_at: parse_timestamp(row, 2)?,
                    classification,
                    state_restore_request_digest: row.get(4)?,
                    state_restore_payload_digest: row.get(5)?,
                    state_restore_before_canonical_digest: row.get(6)?,
                    state_restore_after_canonical_digest: row.get(7)?,
                    state_restore_outbox_event_id: row.get(8)?,
                    state_projection_delivery_state,
                },
                recorded_at: parse_timestamp(row, 10)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let target_count = u32::try_from(target_count)?;
    if usize::try_from(target_count)? != owners.len() {
        anyhow::bail!("governed data-import target count drift");
    }
    validate_governed_import_operation_id(operation_id)?;
    validate_digest(&payload_digest)?;
    validate_digest(&request_digest)?;
    validate_governed_import_owners(
        &owners
            .iter()
            .map(|owner| GovernedDataImportOwnerPlan {
                owner: owner.owner.clone(),
                import_target: owner.import_target.clone(),
                before_digest: owner.before_digest.clone(),
                target_digest: owner.target_digest.clone(),
                item_count: owner.item_count,
            })
            .collect::<Vec<_>>(),
    )?;
    let stage = GovernedDataImportStage::parse(&stage_raw)?;
    if stage == GovernedDataImportStage::AbandonedPreservingCurrent {
        let resolutions = resolution_evidence
            .iter()
            .map(|evidence| evidence.resolution.clone())
            .collect::<Vec<_>>();
        let observations = resolutions
            .iter()
            .map(|resolution| GovernedDataImportOwnerObservation {
                owner: resolution.owner.clone(),
                observed_digest: resolution.observed_digest.clone(),
                observed_at: resolution.observed_at,
                state_restore_request_digest: resolution.state_restore_request_digest.clone(),
                state_restore_payload_digest: resolution.state_restore_payload_digest.clone(),
                state_restore_before_canonical_digest: resolution
                    .state_restore_before_canonical_digest
                    .clone(),
                state_restore_after_canonical_digest: resolution
                    .state_restore_after_canonical_digest
                    .clone(),
                state_restore_outbox_event_id: resolution.state_restore_outbox_event_id.clone(),
                state_projection_delivery_state: resolution.state_projection_delivery_state,
            })
            .collect::<Vec<_>>();
        validate_governed_import_observations(&observations)?;
        let derived = derive_governed_import_resolution_facts(
            operation_id,
            &request_digest,
            &owners,
            &observations,
            governed_import_state_commit_was_recorded(conn, operation_id)?,
        )?;
        if derived != resolutions {
            return Err(GovernedDataImportDrift {
                operation_id: operation_id.to_string(),
                field: "resolution_evidence_derived_classification".into(),
            }
            .into());
        }
    } else if !resolution_evidence.is_empty() {
        anyhow::bail!("governed data-import non-abandoned operation has resolution evidence");
    }
    let terminal_at = terminal_at
        .map(|value| DateTime::parse_from_rfc3339(&value).map(|time| time.with_timezone(&Utc)))
        .transpose()?;
    if stage.is_terminal() != terminal_at.is_some() {
        anyhow::bail!("governed data-import terminal timestamp drift");
    }
    Ok(Some(GovernedDataImportReceipt {
        operation_id: operation_id.to_string(),
        payload_digest,
        request_digest,
        stage,
        target_count,
        owners,
        resolution_evidence,
        created_at: DateTime::parse_from_rfc3339(&created_at)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at)?.with_timezone(&Utc),
        terminal_at,
    }))
}

fn parse_governed_import_stage_sql(
    index: usize,
    value: &str,
) -> rusqlite::Result<GovernedDataImportStage> {
    GovernedDataImportStage::parse(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.to_string(),
            )),
        )
    })
}

fn parse_governed_import_owner_status_sql(
    index: usize,
    value: &str,
) -> rusqlite::Result<GovernedDataImportOwnerStatus> {
    GovernedDataImportOwnerStatus::parse(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.to_string(),
            )),
        )
    })
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

    fn governed_import_plan(
        operation_id: String,
        payload_label: &str,
    ) -> GovernedDataImportPrepare {
        GovernedDataImportPrepare {
            operation_id,
            payload_digest: metadata_digest(payload_label),
            request_digest: metadata_digest("governed manual restore request"),
            owners: vec![
                GovernedDataImportOwnerPlan {
                    owner: "VectorStore".into(),
                    import_target: "vectors".into(),
                    before_digest: metadata_digest("vectors-before"),
                    target_digest: metadata_digest("vectors-target"),
                    item_count: 3,
                },
                GovernedDataImportOwnerPlan {
                    owner: "LifeModelFileStore".into(),
                    import_target: "life_model".into(),
                    before_digest: metadata_digest("lifemodel-before"),
                    target_digest: metadata_digest("lifemodel-target"),
                    item_count: 1,
                },
                GovernedDataImportOwnerPlan {
                    owner: "StateStore".into(),
                    import_target: "state_store".into(),
                    before_digest: metadata_digest("state-before"),
                    target_digest: metadata_digest("state-target"),
                    item_count: 2,
                },
                GovernedDataImportOwnerPlan {
                    owner: "MemoryStore".into(),
                    import_target: "messages".into(),
                    before_digest: metadata_digest("messages-before"),
                    target_digest: metadata_digest("messages-target"),
                    item_count: 4,
                },
            ],
        }
    }

    fn governed_import_abandonment_resolutions(
        receipt: &GovernedDataImportReceipt,
    ) -> Vec<GovernedDataImportOwnerObservation> {
        let observed_at = Utc::now();
        receipt
            .owners
            .iter()
            .map(|owner| {
                let observed_digest = match owner.import_target.as_str() {
                    "life_model" => owner.before_digest.clone(),
                    "messages" => owner.target_digest.clone(),
                    "state_store" => metadata_digest("state-after-canonical"),
                    "vectors" => metadata_digest("vectors-observed-third-state"),
                    target => panic!("unexpected governed import target {target}"),
                };
                let is_state = owner.import_target == "state_store";
                GovernedDataImportOwnerObservation {
                    owner: owner.owner.clone(),
                    observed_digest,
                    observed_at,
                    state_restore_request_digest: is_state.then(|| receipt.request_digest.clone()),
                    state_restore_payload_digest: is_state.then(|| owner.target_digest.clone()),
                    state_restore_before_canonical_digest: is_state
                        .then(|| owner.before_digest.clone()),
                    state_restore_after_canonical_digest: is_state
                        .then(|| metadata_digest("state-after-canonical")),
                    state_restore_outbox_event_id: is_state
                        .then(|| format!("outbox:{}", Uuid::new_v4())),
                    state_projection_delivery_state: is_state
                        .then_some(ProjectionDeliveryState::Applied),
                }
            })
            .collect()
    }

    #[test]
    fn governed_import_journal_schema_reopens_with_metadata_only_prepared_receipt() {
        const RAW_PRIVATE_SENTINEL: &str = "private import body must never enter saga journal";
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("journal.db");
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let expected_payload_digest = metadata_digest(RAW_PRIVATE_SENTINEL);
        {
            let journal = GovernedDataImportJournal::new(&path).unwrap();
            let outcome = journal
                .prepare(governed_import_plan(
                    operation_id.clone(),
                    RAW_PRIVATE_SENTINEL,
                ))
                .unwrap();
            let receipt = outcome.receipt;
            assert_eq!(receipt.stage, GovernedDataImportStage::Prepared);
            assert_eq!(receipt.payload_digest, expected_payload_digest);
            assert_eq!(receipt.target_count, 4);
            assert_eq!(receipt.owners.len(), 4);
            assert!(receipt.terminal_at.is_none());
        }

        let reopened = GovernedDataImportJournal::new(&path).unwrap();
        let receipt = reopened.receipt(&operation_id).unwrap().unwrap();
        assert_eq!(receipt.stage, GovernedDataImportStage::Prepared);
        assert_eq!(receipt.payload_digest, expected_payload_digest);
        let reopened_prepare = reopened
            .prepare(governed_import_plan(
                operation_id.clone(),
                RAW_PRIVATE_SENTINEL,
            ))
            .unwrap();
        assert!(reopened_prepare.replayed);
        assert_eq!(reopened_prepare.receipt, receipt);
        assert_eq!(
            reopened
                .recovery_requirement()
                .unwrap()
                .unwrap()
                .operation_id,
            operation_id
        );
        let conn = Connection::open(&path).unwrap();
        let persisted_private_body_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM governed_data_import_operations
                 WHERE payload_digest = ?1 OR request_digest = ?1",
                [RAW_PRIVATE_SENTINEL],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(persisted_private_body_count, 0);
    }

    #[test]
    fn governed_import_prepare_replays_exact_metadata_and_rejects_digest_drift() {
        let directory = tempfile::tempdir().unwrap();
        let journal = GovernedDataImportJournal::new(directory.path().join("journal.db")).unwrap();
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let input = governed_import_plan(operation_id.clone(), "payload-one");
        let first = journal.prepare(input.clone()).unwrap();
        let replayed = journal.prepare(input).unwrap();
        assert!(!first.replayed);
        assert!(replayed.replayed);
        assert_eq!(first.receipt, replayed.receipt);
        assert_eq!(journal.transitions(&operation_id).unwrap().len(), 1);

        let error = journal
            .prepare(governed_import_plan(operation_id, "payload-two"))
            .unwrap_err();
        let drift = error
            .downcast_ref::<GovernedDataImportDrift>()
            .expect("same operation with changed digest must remain typed drift");
        assert_eq!(drift.field, "payload_digest");
    }

    #[test]
    fn governed_import_transition_is_monotonic_and_terminal_receipt_is_durable() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("journal.db");
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let journal = GovernedDataImportJournal::new(&path).unwrap();
        journal
            .prepare(governed_import_plan(operation_id.clone(), "payload"))
            .unwrap();

        let illegal = journal
            .transition(
                &operation_id,
                GovernedDataImportStage::VectorApplied,
                &[],
                None,
            )
            .unwrap_err();
        assert!(illegal
            .downcast_ref::<GovernedDataImportInvalidTransition>()
            .is_some());

        let steps = [
            (
                GovernedDataImportStage::LifeModelApplied,
                "LifeModelFileStore",
            ),
            (GovernedDataImportStage::MemoryApplied, "MemoryStore"),
            (GovernedDataImportStage::VectorApplied, "VectorStore"),
            (GovernedDataImportStage::StateCommitted, "StateStore"),
        ];
        for (stage, owner) in steps {
            let update = GovernedDataImportOwnerUpdate {
                owner: owner.into(),
                status: GovernedDataImportOwnerStatus::Applied,
            };
            let receipt = journal
                .transition(&operation_id, stage, std::slice::from_ref(&update), None)
                .unwrap();
            assert_eq!(receipt.stage, stage);
            let replayed = journal
                .transition(&operation_id, stage, &[update], None)
                .unwrap();
            assert_eq!(replayed, receipt);
        }
        let missing_projection_evidence = journal
            .transition(&operation_id, GovernedDataImportStage::Completed, &[], None)
            .unwrap_err();
        assert!(missing_projection_evidence
            .to_string()
            .contains("requires a projection evidence digest"));
        assert_eq!(
            journal.receipt(&operation_id).unwrap().unwrap().stage,
            GovernedDataImportStage::StateCommitted,
        );
        let terminal = journal
            .transition(
                &operation_id,
                GovernedDataImportStage::Completed,
                &[],
                Some(&metadata_digest("state projection applied receipt")),
            )
            .unwrap();
        assert_eq!(terminal.stage, GovernedDataImportStage::Completed);
        assert!(terminal.terminal_at.is_some());
        assert_eq!(
            journal.terminal_receipt(&operation_id).unwrap(),
            Some(terminal)
        );
        assert!(journal.recovery_requirement().unwrap().is_none());
        assert_eq!(journal.transitions(&operation_id).unwrap().len(), 6);

        drop(journal);
        let reopened = GovernedDataImportJournal::new(&path).unwrap();
        assert_eq!(
            reopened
                .terminal_receipt(&operation_id)
                .unwrap()
                .unwrap()
                .stage,
            GovernedDataImportStage::Completed
        );
    }

    #[test]
    fn governed_import_projection_degraded_cannot_complete_without_recovery_evidence() {
        let directory = tempfile::tempdir().unwrap();
        let journal = GovernedDataImportJournal::new(directory.path().join("journal.db")).unwrap();
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        journal
            .prepare(governed_import_plan(operation_id.clone(), "payload"))
            .unwrap();
        for (stage, owner) in [
            (
                GovernedDataImportStage::LifeModelApplied,
                "LifeModelFileStore",
            ),
            (GovernedDataImportStage::MemoryApplied, "MemoryStore"),
            (GovernedDataImportStage::VectorApplied, "VectorStore"),
            (GovernedDataImportStage::StateCommitted, "StateStore"),
        ] {
            journal
                .transition(
                    &operation_id,
                    stage,
                    &[GovernedDataImportOwnerUpdate {
                        owner: owner.into(),
                        status: GovernedDataImportOwnerStatus::Applied,
                    }],
                    None,
                )
                .unwrap();
        }
        journal
            .transition(
                &operation_id,
                GovernedDataImportStage::ProjectionDegraded,
                &[],
                Some(&metadata_digest(
                    "projection failed before durable acknowledgement",
                )),
            )
            .unwrap();

        let missing_recovery_evidence = journal
            .transition(&operation_id, GovernedDataImportStage::Completed, &[], None)
            .unwrap_err();
        assert!(missing_recovery_evidence
            .to_string()
            .contains("requires a projection evidence digest"));
        assert_eq!(
            journal.receipt(&operation_id).unwrap().unwrap().stage,
            GovernedDataImportStage::ProjectionDegraded,
        );
        assert_eq!(
            journal
                .transition(
                    &operation_id,
                    GovernedDataImportStage::Completed,
                    &[],
                    Some(&metadata_digest("recovered projection applied receipt")),
                )
                .unwrap()
                .stage,
            GovernedDataImportStage::Completed,
        );
    }

    #[test]
    fn governed_import_without_state_target_can_complete_without_projection_evidence() {
        let directory = tempfile::tempdir().unwrap();
        let journal = GovernedDataImportJournal::new(directory.path().join("journal.db")).unwrap();
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let mut input = governed_import_plan(operation_id.clone(), "payload");
        input
            .owners
            .retain(|owner| owner.import_target == "life_model");
        journal.prepare(input).unwrap();
        journal
            .transition(
                &operation_id,
                GovernedDataImportStage::LifeModelApplied,
                &[GovernedDataImportOwnerUpdate {
                    owner: "LifeModelFileStore".into(),
                    status: GovernedDataImportOwnerStatus::Applied,
                }],
                None,
            )
            .unwrap();
        for stage in [
            GovernedDataImportStage::MemoryApplied,
            GovernedDataImportStage::VectorApplied,
            GovernedDataImportStage::StateCommitted,
        ] {
            journal.transition(&operation_id, stage, &[], None).unwrap();
        }

        assert_eq!(
            journal
                .transition(&operation_id, GovernedDataImportStage::Completed, &[], None,)
                .unwrap()
                .stage,
            GovernedDataImportStage::Completed,
        );
    }

    #[test]
    fn governed_import_compensation_unknown_blocks_until_exact_compensation_receipt() {
        let directory = tempfile::tempdir().unwrap();
        let journal = GovernedDataImportJournal::new(directory.path().join("journal.db")).unwrap();
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let mut input = governed_import_plan(operation_id.clone(), "payload");
        input
            .owners
            .retain(|owner| owner.import_target == "life_model");
        let owner = input.owners[0].owner.clone();
        journal.prepare(input).unwrap();
        let reason_digest = metadata_digest("compensation durability unknown");
        let unknown = journal
            .transition(
                &operation_id,
                GovernedDataImportStage::CompensationUnknown,
                &[GovernedDataImportOwnerUpdate {
                    owner: owner.clone(),
                    status: GovernedDataImportOwnerStatus::Unknown,
                }],
                Some(&reason_digest),
            )
            .unwrap();
        assert_eq!(unknown.stage, GovernedDataImportStage::CompensationUnknown);
        assert!(journal.terminal_receipt(&operation_id).unwrap().is_none());
        assert_eq!(
            journal.recovery_requirement().unwrap().unwrap().stage,
            GovernedDataImportStage::CompensationUnknown
        );

        let compensated = journal
            .transition(
                &operation_id,
                GovernedDataImportStage::Compensated,
                &[GovernedDataImportOwnerUpdate {
                    owner,
                    status: GovernedDataImportOwnerStatus::Compensated,
                }],
                Some(&metadata_digest(
                    "manual digest reconciliation proved restore",
                )),
            )
            .unwrap();
        assert_eq!(compensated.stage, GovernedDataImportStage::Compensated);
        assert!(compensated.terminal_at.is_some());
        assert!(journal.recovery_requirement().unwrap().is_none());
    }

    #[test]
    fn governed_import_abandonment_atomically_preserves_owner_status_and_clears_recovery() {
        const RAW_PRIVATE_SENTINEL: &str =
            "private import body must not enter abandonment resolution evidence";
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("journal.db");
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let input = governed_import_plan(operation_id.clone(), RAW_PRIVATE_SENTINEL);
        let journal = GovernedDataImportJournal::new(&path).unwrap();
        let prepared = journal.prepare(input.clone()).unwrap().receipt;
        journal
            .transition(
                &operation_id,
                GovernedDataImportStage::LifeModelApplied,
                &[GovernedDataImportOwnerUpdate {
                    owner: "LifeModelFileStore".into(),
                    status: GovernedDataImportOwnerStatus::Applied,
                }],
                None,
            )
            .unwrap();
        let unknown_reason = metadata_digest("owner compensation could not be reconstructed");
        let unresolved = journal
            .transition(
                &operation_id,
                GovernedDataImportStage::CompensationUnknown,
                &[GovernedDataImportOwnerUpdate {
                    owner: "MemoryStore".into(),
                    status: GovernedDataImportOwnerStatus::Unknown,
                }],
                Some(&unknown_reason),
            )
            .unwrap();
        let resolutions = governed_import_abandonment_resolutions(&prepared);
        let abandonment_reason = metadata_digest("payload unavailable owner facts observed");
        let abandoned = journal
            .abandon_preserving_current(&operation_id, &resolutions, &abandonment_reason)
            .unwrap();

        assert_eq!(
            abandoned.stage,
            GovernedDataImportStage::AbandonedPreservingCurrent
        );
        assert!(abandoned.terminal_at.is_some());
        assert_eq!(abandoned.resolution_evidence.len(), 4);
        assert!(journal.recovery_requirement().unwrap().is_none());
        assert_eq!(journal.latest_receipt().unwrap(), Some(abandoned.clone()));
        assert_eq!(
            abandoned
                .owners
                .iter()
                .map(|owner| (&owner.owner, owner.status))
                .collect::<Vec<_>>(),
            unresolved
                .owners
                .iter()
                .map(|owner| (&owner.owner, owner.status))
                .collect::<Vec<_>>()
        );

        let replayed_abandonment = journal
            .abandon_preserving_current(&operation_id, &resolutions, &abandonment_reason)
            .unwrap();
        assert_eq!(replayed_abandonment, abandoned);
        let replayed_prepare = journal.prepare(input).unwrap();
        assert!(replayed_prepare.replayed);
        assert_eq!(replayed_prepare.receipt, abandoned);
        assert_eq!(
            journal
                .transitions(&operation_id)
                .unwrap()
                .last()
                .unwrap()
                .to_stage,
            GovernedDataImportStage::AbandonedPreservingCurrent
        );

        let conn = Connection::open(&path).unwrap();
        let raw_body_count: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM governed_data_import_operations operations
                 LEFT JOIN governed_data_import_resolution_evidence evidence
                   ON evidence.operation_id = operations.operation_id
                 WHERE operations.payload_digest = ?1
                    OR operations.request_digest = ?1
                    OR evidence.observed_digest = ?1
                    OR evidence.state_restore_request_digest = ?1
                    OR evidence.state_restore_payload_digest = ?1
                    OR evidence.state_restore_before_canonical_digest = ?1
                    OR evidence.state_restore_after_canonical_digest = ?1
                    OR evidence.state_restore_outbox_event_id = ?1",
                [RAW_PRIVATE_SENTINEL],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raw_body_count, 0);
    }

    #[test]
    fn governed_import_abandonment_rejects_missing_or_tampered_resolution_evidence() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("journal.db");
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let journal = GovernedDataImportJournal::new(&path).unwrap();
        let prepared = journal
            .prepare(governed_import_plan(operation_id.clone(), "payload"))
            .unwrap()
            .receipt;
        let reason = metadata_digest("payload unavailable");
        let mut resolutions = governed_import_abandonment_resolutions(&prepared);
        resolutions.pop();
        let missing = journal
            .abandon_preserving_current(&operation_id, &resolutions, &reason)
            .unwrap_err();
        assert_eq!(
            missing
                .downcast_ref::<GovernedDataImportDrift>()
                .unwrap()
                .field,
            "resolution_owner_coverage"
        );
        assert_eq!(
            journal.receipt(&operation_id).unwrap().unwrap().stage,
            GovernedDataImportStage::Prepared
        );

        let mut resolutions = governed_import_abandonment_resolutions(&prepared);
        let state_resolution = resolutions
            .iter_mut()
            .find(|resolution| resolution.owner == "StateStore")
            .unwrap();
        state_resolution.state_restore_payload_digest =
            Some(metadata_digest("tampered portable payload binding"));
        let tampered = journal
            .abandon_preserving_current(&operation_id, &resolutions, &reason)
            .unwrap_err();
        assert!(tampered
            .downcast_ref::<GovernedDataImportDrift>()
            .unwrap()
            .field
            .contains("StateStore restore binding"));
        let evidence_count: i64 = Connection::open(&path)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM governed_data_import_resolution_evidence",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            evidence_count, 0,
            "failed evidence must roll back atomically"
        );

        let resolutions = governed_import_abandonment_resolutions(&prepared);
        journal
            .abandon_preserving_current(&operation_id, &resolutions, &reason)
            .unwrap();
        let conn = Connection::open(&path).unwrap();
        conn.execute(
            "UPDATE governed_data_import_resolution_evidence
             SET classification = 'before'
             WHERE operation_id = ?1 AND owner = 'MemoryStore'",
            [&operation_id],
        )
        .unwrap();
        let read_error = journal.receipt(&operation_id).unwrap_err();
        assert!(read_error
            .downcast_ref::<GovernedDataImportDrift>()
            .is_some());
        drop(conn);
        drop(journal);
        assert!(
            GovernedDataImportJournal::new(&path).is_err(),
            "journal admission must fail closed on a tampered abandoned terminal graph"
        );
    }

    #[test]
    fn governed_import_abandonment_rolls_back_when_existing_transition_graph_is_invalid() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("journal.db");
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let journal = GovernedDataImportJournal::new(&path).unwrap();
        let prepared = journal
            .prepare(governed_import_plan(operation_id.clone(), "payload"))
            .unwrap()
            .receipt;
        Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE governed_data_import_transitions
                 SET from_stage = 'lifemodel_applied'
                 WHERE operation_id = ?1 AND sequence = 0",
                [&operation_id],
            )
            .unwrap();

        let error = journal
            .abandon_preserving_current(
                &operation_id,
                &governed_import_abandonment_resolutions(&prepared),
                &metadata_digest("must not commit over malformed graph"),
            )
            .unwrap_err();
        assert!(error.to_string().contains("transition root is invalid"));
        assert_eq!(
            journal.receipt(&operation_id).unwrap().unwrap().stage,
            GovernedDataImportStage::Prepared
        );
        let conn = Connection::open(&path).unwrap();
        let evidence_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM governed_data_import_resolution_evidence
                 WHERE operation_id = ?1",
                [&operation_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            evidence_count, 0,
            "failed terminal graph must roll back evidence"
        );
    }

    #[test]
    fn governed_import_abandonment_allows_state_store_observed_before_commit_without_outbox() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("journal.db");
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let journal = GovernedDataImportJournal::new(&path).unwrap();
        let prepared = journal
            .prepare(governed_import_plan(operation_id.clone(), "payload"))
            .unwrap()
            .receipt;
        let mut resolutions = governed_import_abandonment_resolutions(&prepared);
        let state_owner = prepared
            .owners
            .iter()
            .find(|owner| owner.import_target == "state_store")
            .unwrap();
        let state_resolution = resolutions
            .iter_mut()
            .find(|resolution| resolution.owner == state_owner.owner)
            .unwrap();
        state_resolution.observed_digest = state_owner.before_digest.clone();
        state_resolution.state_restore_request_digest = None;
        state_resolution.state_restore_payload_digest = None;
        state_resolution.state_restore_before_canonical_digest = None;
        state_resolution.state_restore_after_canonical_digest = None;
        state_resolution.state_restore_outbox_event_id = None;
        state_resolution.state_projection_delivery_state = None;

        let abandoned = journal
            .abandon_preserving_current(
                &operation_id,
                &resolutions,
                &metadata_digest("state restore receipt observed absent before commit"),
            )
            .unwrap();

        assert_eq!(
            abandoned.stage,
            GovernedDataImportStage::AbandonedPreservingCurrent
        );
        let evidence = abandoned
            .resolution_evidence
            .iter()
            .find(|evidence| evidence.resolution.owner == state_owner.owner)
            .unwrap();
        assert_eq!(
            evidence.resolution.classification,
            GovernedDataImportResolutionClassification::Before
        );
        assert!(evidence.resolution.state_restore_outbox_event_id.is_none());
        assert!(evidence
            .resolution
            .state_projection_delivery_state
            .is_none());
        assert!(journal.recovery_requirement().unwrap().is_none());
    }

    #[test]
    fn governed_import_abandonment_requires_state_receipt_after_recorded_commit() {
        let directory = tempfile::tempdir().unwrap();
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let journal = GovernedDataImportJournal::new(directory.path().join("journal.db")).unwrap();
        let prepared = journal
            .prepare(governed_import_plan(operation_id.clone(), "payload"))
            .unwrap()
            .receipt;
        for (stage, owner) in [
            (
                GovernedDataImportStage::LifeModelApplied,
                "LifeModelFileStore",
            ),
            (GovernedDataImportStage::MemoryApplied, "MemoryStore"),
            (GovernedDataImportStage::VectorApplied, "VectorStore"),
            (GovernedDataImportStage::StateCommitted, "StateStore"),
        ] {
            journal
                .transition(
                    &operation_id,
                    stage,
                    &[GovernedDataImportOwnerUpdate {
                        owner: owner.into(),
                        status: GovernedDataImportOwnerStatus::Applied,
                    }],
                    None,
                )
                .unwrap();
        }
        let mut observations = governed_import_abandonment_resolutions(&prepared);
        let state = observations
            .iter_mut()
            .find(|observation| observation.owner == "StateStore")
            .unwrap();
        state.observed_digest = prepared
            .owners
            .iter()
            .find(|owner| owner.owner == "StateStore")
            .unwrap()
            .before_digest
            .clone();
        state.state_restore_request_digest = None;
        state.state_restore_payload_digest = None;
        state.state_restore_before_canonical_digest = None;
        state.state_restore_after_canonical_digest = None;
        state.state_restore_outbox_event_id = None;
        state.state_projection_delivery_state = None;

        let error = journal
            .abandon_preserving_current(
                &operation_id,
                &observations,
                &metadata_digest("missing StateStore receipt after recorded commit"),
            )
            .unwrap_err();
        assert!(error
            .downcast_ref::<GovernedDataImportDrift>()
            .unwrap()
            .field
            .contains("missing committed StateStore proof"));
        assert_eq!(
            journal.receipt(&operation_id).unwrap().unwrap().stage,
            GovernedDataImportStage::StateCommitted
        );
    }

    #[test]
    fn governed_import_abandonment_classifies_state_drift_after_exact_restore_as_other() {
        let directory = tempfile::tempdir().unwrap();
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let journal = GovernedDataImportJournal::new(directory.path().join("journal.db")).unwrap();
        let prepared = journal
            .prepare(governed_import_plan(operation_id.clone(), "payload"))
            .unwrap()
            .receipt;
        let mut observations = governed_import_abandonment_resolutions(&prepared);
        let state = observations
            .iter_mut()
            .find(|observation| observation.owner == "StateStore")
            .unwrap();
        state.observed_digest = metadata_digest("state-late-canonical-drift");

        let abandoned = journal
            .abandon_preserving_current(
                &operation_id,
                &observations,
                &metadata_digest("preserve late StateStore drift"),
            )
            .unwrap();
        assert_eq!(
            abandoned
                .resolution_evidence
                .iter()
                .find(|evidence| evidence.resolution.owner == "StateStore")
                .unwrap()
                .resolution
                .classification,
            GovernedDataImportResolutionClassification::Other
        );
    }

    #[test]
    fn governed_import_intermediate_resolution_evidence_schema_adds_state_delivery_proof_columns() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("journal.db");
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let observed_digest = metadata_digest("preserved intermediate evidence");
        {
            let journal = GovernedDataImportJournal::new(&path).unwrap();
            let mut input = governed_import_plan(operation_id.clone(), "payload");
            input
                .owners
                .retain(|owner| owner.import_target == "life_model");
            journal.prepare(input).unwrap();
            journal
                .abandon_preserving_current(
                    &operation_id,
                    &[GovernedDataImportOwnerObservation {
                        owner: "LifeModelFileStore".into(),
                        observed_digest: observed_digest.clone(),
                        observed_at: Utc::now(),
                        state_restore_request_digest: None,
                        state_restore_payload_digest: None,
                        state_restore_before_canonical_digest: None,
                        state_restore_after_canonical_digest: None,
                        state_restore_outbox_event_id: None,
                        state_projection_delivery_state: None,
                    }],
                    &metadata_digest("preserve current intermediate evidence"),
                )
                .unwrap();
        }
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
            conn.execute_batch(
                "ALTER TABLE governed_data_import_resolution_evidence
                    RENAME TO governed_data_import_resolution_evidence_current;
                 CREATE TABLE governed_data_import_resolution_evidence (
                    operation_id TEXT NOT NULL,
                    owner TEXT NOT NULL,
                    observed_digest TEXT NOT NULL,
                    observed_at TEXT NOT NULL,
                    classification TEXT NOT NULL CHECK(classification IN (
                        'before', 'target', 'other'
                    )),
                    state_restore_request_digest TEXT,
                    state_restore_payload_digest TEXT,
                    state_restore_before_canonical_digest TEXT,
                    state_restore_after_canonical_digest TEXT,
                    recorded_at TEXT NOT NULL,
                    PRIMARY KEY(operation_id, owner),
                    CHECK(
                        (state_restore_request_digest IS NULL AND
                         state_restore_payload_digest IS NULL AND
                         state_restore_before_canonical_digest IS NULL AND
                         state_restore_after_canonical_digest IS NULL) OR
                        (state_restore_request_digest IS NOT NULL AND
                         state_restore_payload_digest IS NOT NULL AND
                         state_restore_before_canonical_digest IS NOT NULL AND
                         state_restore_after_canonical_digest IS NOT NULL)
                    ),
                    FOREIGN KEY(operation_id, owner)
                        REFERENCES governed_data_import_owners(operation_id, owner)
                        ON DELETE CASCADE
                 ) WITHOUT ROWID;
                 INSERT INTO governed_data_import_resolution_evidence (
                    operation_id, owner, observed_digest, observed_at,
                    classification, state_restore_request_digest,
                    state_restore_payload_digest,
                    state_restore_before_canonical_digest,
                    state_restore_after_canonical_digest, recorded_at
                 )
                 SELECT operation_id, owner, observed_digest, observed_at,
                        classification, state_restore_request_digest,
                        state_restore_payload_digest,
                        state_restore_before_canonical_digest,
                        state_restore_after_canonical_digest, recorded_at
                 FROM governed_data_import_resolution_evidence_current;
                 DROP TABLE governed_data_import_resolution_evidence_current;",
            )
            .unwrap();
            conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        }

        for _ in 0..2 {
            let reopened = GovernedDataImportJournal::new(&path).unwrap();
            let receipt = reopened.receipt(&operation_id).unwrap().unwrap();
            assert_eq!(
                receipt.stage,
                GovernedDataImportStage::AbandonedPreservingCurrent
            );
            assert_eq!(receipt.resolution_evidence.len(), 1);
            let evidence = &receipt.resolution_evidence[0];
            assert_eq!(evidence.resolution.observed_digest, observed_digest);
            assert!(evidence.resolution.state_restore_outbox_event_id.is_none());
            assert!(evidence
                .resolution
                .state_projection_delivery_state
                .is_none());
        }

        let conn = Connection::open(&path).unwrap();
        let columns = conn
            .prepare("PRAGMA table_info(governed_data_import_resolution_evidence)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            columns
                .iter()
                .filter(|column| column.as_str() == "state_restore_outbox_event_id")
                .count(),
            1
        );
        assert_eq!(
            columns
                .iter()
                .filter(|column| column.as_str() == "state_projection_delivery_state")
                .count(),
            1
        );
        assert!(conn
            .execute(
                "UPDATE governed_data_import_resolution_evidence
                 SET state_projection_delivery_state = 'invalid'
                 WHERE operation_id = ?1",
                [&operation_id],
            )
            .is_err());
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM governed_data_import_resolution_evidence
                 WHERE operation_id = ?1 AND observed_digest = ?2",
                params![operation_id, observed_digest],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn governed_import_old_schema_migrates_transactionally_and_can_be_abandoned() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("journal.db");
        let operation_id = Uuid::new_v4().hyphenated().to_string();
        let created_at = Utc::now().to_rfc3339();
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "foreign_keys", "ON").unwrap();
            conn.execute_batch(
                "CREATE TABLE governed_data_import_operations (
                    operation_id TEXT PRIMARY KEY,
                    payload_digest TEXT NOT NULL,
                    request_digest TEXT NOT NULL,
                    stage TEXT NOT NULL CHECK(stage IN (
                        'prepared', 'lifemodel_applied', 'memory_applied',
                        'vector_applied', 'state_committed', 'projection_degraded',
                        'completed', 'compensated', 'compensation_unknown'
                    )),
                    target_count INTEGER NOT NULL CHECK(target_count > 0),
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    terminal_at TEXT
                 );
                 CREATE UNIQUE INDEX idx_governed_data_import_one_unresolved
                 ON governed_data_import_operations((1))
                 WHERE stage NOT IN ('completed', 'compensated');
                 CREATE TABLE governed_data_import_owners (
                    operation_id TEXT NOT NULL,
                    owner TEXT NOT NULL,
                    import_target TEXT NOT NULL,
                    before_digest TEXT NOT NULL,
                    target_digest TEXT NOT NULL,
                    item_count INTEGER NOT NULL CHECK(item_count >= 0),
                    status TEXT NOT NULL CHECK(status IN (
                        'pending', 'applied', 'skipped', 'compensated', 'unknown'
                    )),
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    PRIMARY KEY(operation_id, owner),
                    UNIQUE(operation_id, import_target),
                    FOREIGN KEY(operation_id)
                        REFERENCES governed_data_import_operations(operation_id)
                        ON DELETE CASCADE
                 ) WITHOUT ROWID;
                 CREATE TABLE governed_data_import_transitions (
                    operation_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL CHECK(sequence >= 0),
                    from_stage TEXT,
                    to_stage TEXT NOT NULL,
                    reason_digest TEXT,
                    created_at TEXT NOT NULL,
                    PRIMARY KEY(operation_id, sequence),
                    FOREIGN KEY(operation_id)
                        REFERENCES governed_data_import_operations(operation_id)
                        ON DELETE CASCADE
                 ) WITHOUT ROWID;",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO governed_data_import_operations (
                    operation_id, payload_digest, request_digest, stage,
                    target_count, created_at, updated_at, terminal_at
                 ) VALUES (?1, ?2, ?3, 'compensation_unknown', 1, ?4, ?4, NULL)",
                params![
                    operation_id,
                    metadata_digest("legacy payload"),
                    metadata_digest("legacy request"),
                    created_at,
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO governed_data_import_owners (
                    operation_id, owner, import_target, before_digest,
                    target_digest, item_count, status, created_at, updated_at
                 ) VALUES (?1, 'LifeModelFileStore', 'life_model', ?2, ?3,
                           1, 'unknown', ?4, ?4)",
                params![
                    operation_id,
                    metadata_digest("legacy before"),
                    metadata_digest("legacy target"),
                    created_at,
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO governed_data_import_transitions (
                    operation_id, sequence, from_stage, to_stage,
                    reason_digest, created_at
                 ) VALUES (?1, 0, NULL, 'compensation_unknown', ?2, ?3)",
                params![
                    operation_id,
                    metadata_digest("legacy unknown reason"),
                    created_at,
                ],
            )
            .unwrap();
        }

        let journal = GovernedDataImportJournal::new(&path).unwrap();
        let receipt = journal.receipt(&operation_id).unwrap().unwrap();
        assert_eq!(receipt.stage, GovernedDataImportStage::CompensationUnknown);
        let resolution = GovernedDataImportOwnerObservation {
            owner: "LifeModelFileStore".into(),
            observed_digest: metadata_digest("legacy observed third state"),
            observed_at: Utc::now(),
            state_restore_request_digest: None,
            state_restore_payload_digest: None,
            state_restore_before_canonical_digest: None,
            state_restore_after_canonical_digest: None,
            state_restore_outbox_event_id: None,
            state_projection_delivery_state: None,
        };
        let abandoned = journal
            .abandon_preserving_current(
                &operation_id,
                &[resolution],
                &metadata_digest("legacy payload unavailable"),
            )
            .unwrap();
        assert_eq!(
            abandoned.stage,
            GovernedDataImportStage::AbandonedPreservingCurrent
        );
        assert!(journal.recovery_requirement().unwrap().is_none());
        drop(journal);
        let reopened = GovernedDataImportJournal::new(&path).unwrap();
        assert_eq!(
            reopened
                .terminal_receipt(&operation_id)
                .unwrap()
                .unwrap()
                .stage,
            GovernedDataImportStage::AbandonedPreservingCurrent
        );
        let schema: String = Connection::open(&path)
            .unwrap()
            .query_row(
                "SELECT sql FROM sqlite_master
                 WHERE type = 'table' AND name = 'governed_data_import_operations'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(schema.contains("'abandoned_preserving_current'"));
    }
}
