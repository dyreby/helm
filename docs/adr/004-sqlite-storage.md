# ADR 004: SQLite storage pivot

**Status:** Accepted
**Date:** 2026-02-27
**Issues:** [#129](https://github.com/dyreby/helm/issues/129), [#127](https://github.com/dyreby/helm/issues/127)

## Context

Helm's storage model — `voyage.json`, `slate.jsonl`, `logbook.jsonl`, and a `hold/` directory — was built as the simplest thing that would work. After implementing slate management (#118), several gaps became friction:

**The slate is a set, stored as an append log.** ADR 002 established one observation per target as an invariant. JSONL appends fight this: observe the same target twice, both entries land in the file. Set operations (upsert, erase) require read-all-rewrite-all, which is non-atomic and slow.

**Sealing is multi-step across files.** Writing a logbook entry and clearing the slate are separate operations on separate files. A crash between them leaves the voyage in an inconsistent state — entries sealed but slate not cleared, or vice versa.

**The inline/hold split introduces a threshold decision.** ADR 001 described spilling large payloads to `hold/` to cap bearing size. This threshold decision doesn't belong in the storage layer, and blob GC (orphaned hold entries) requires a filesystem walk against logbook content.

**The hold was never built.** `bearing.rs` documents "large payloads should spill to the hold (deferred — hold storage not yet implemented)." The split adds design debt without delivering the benefit.

**Concurrent write risk.** Multiple agents steering the same voyage can corrupt JSONL files — no locking.

## Decision

Replace the file-based layout with one SQLite file per voyage.

```
~/.helm/voyages/
  <id>.sqlite
```

### Schema

```sql
PRAGMA user_version = 1;
PRAGMA foreign_keys = ON;  -- must be set on every connection; SQLite does not enforce foreign keys by default

CREATE TABLE voyage (
    id           TEXT PRIMARY KEY,
    intent       TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    status       TEXT NOT NULL CHECK(status IN ('active', 'ended')),
    ended_at     TEXT,
    ended_status TEXT
);

CREATE TABLE artifacts (
    hash   TEXT PRIMARY KEY,
    data   BLOB NOT NULL,  -- zstd-compressed payload JSON
    status TEXT NOT NULL DEFAULT 'stowed' CHECK(status IN ('stowed', 'reduced', 'jettisoned'))
);

CREATE TABLE artifact_derivations (
    source_hash  TEXT NOT NULL REFERENCES artifacts(hash),
    derived_hash TEXT NOT NULL REFERENCES artifacts(hash),
    method       TEXT NOT NULL,   -- how the derivation was produced: 'human', 'llm', etc.
    created_at   TEXT NOT NULL,
    PRIMARY KEY (source_hash, derived_hash)
);

CREATE TABLE slate (
    target        TEXT PRIMARY KEY,  -- JSON-serialized Observe variant
    artifact_hash TEXT NOT NULL REFERENCES artifacts(hash),
    observed_at   TEXT NOT NULL
);

CREATE TABLE logbook (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    recorded_at TEXT NOT NULL,
    identity    TEXT NOT NULL,
    action      TEXT NOT NULL,  -- JSON-serialized EntryKind
    summary     TEXT NOT NULL,
    role        TEXT NOT NULL,   -- cognitive framing: 'reviewer', 'coder', 'planner', etc.
    method      TEXT NOT NULL   -- how thinking was done: 'sonnet 4-6, thinking high', 'human', 'pair session', etc.
);

CREATE TABLE bearing_observations (
    logbook_id    INTEGER NOT NULL REFERENCES logbook(id),
    target        TEXT NOT NULL,
    artifact_hash TEXT NOT NULL REFERENCES artifacts(hash),
    observed_at   TEXT NOT NULL
);
```

### Key properties

**Slate is a set.** `INSERT OR REPLACE INTO slate` on observe. Same target observed twice: one row, newest payload wins. Set semantics enforced by the database — no read-rewrite cycle.

**Erase is one statement.** `DELETE FROM slate WHERE target = ?` — no reads, no rewrites.

**Seal is one transaction.** Insert logbook row, copy slate → `bearing_observations`, clear slate. Atomic. Inconsistent state is impossible.

**Artifacts are first-class.** Observation payloads are stored as content-addressed artifacts — zstd-compressed and keyed by SHA-256 hash of the uncompressed JSON. The same payload observed twice stores one artifact. Content addressing serves deduplication; it is not necessarily the permanent identity strategy for sealed artifacts (see artifact lifecycle below).

**Artifact lifecycle.** Artifacts track their own condition: `stowed` (full payload, verifiable hash), `reduced` (payload removed, summary exists via derivation), or `jettisoned` (payload removed, shell only). The `artifact_derivations` table links a reduced artifact to its summary. This supports future `helm artifact reduce` and `helm artifact jettison` commands without schema changes.

**Foreign key enforcement.** `PRAGMA foreign_keys = ON` is set on every connection. SQLite does not enforce foreign key constraints by default. The constraints on `slate.artifact_hash`, `bearing_observations.artifact_hash`, `bearing_observations.logbook_id`, and `artifact_derivations` are load-bearing for data integrity.

**Provenance on log entries.** Each logbook entry records `identity` (who acted), `role` (what cognitive framing was adopted), and `method` (how the thinking was done). All three are required. These axes are orthogonal: identity is the external actor, role is the mindset, method is the engine. All are freeform text. All apply equally to humans and agents.

**Schema versioning from day one.** `PRAGMA user_version = 1` is set on creation. Migrations are added when needed — not before.

**Concurrent write safety.** SQLite's write locking prevents corruption from multiple agents on the same voyage.

### Dependencies

- `rusqlite` with `bundled` feature — builds SQLite from source. C dependency, accepted. Bundled avoids system SQLite version mismatches.
- `zstd` — compression for payloads. C dependency, accepted. Compression ratio matters for large payloads (full repo codebases, GitHub PR diffs).
- `sha2` — SHA-256 hashing for artifact content addressing. Pure Rust.

### Old voyages

JSONL voyages are not migrated. Old `voyages/<id>/` directories are abandoned in place. The new layout uses `voyages/<id>.sqlite`.

## Amends

- **ADR 001**: The hold (`hold/` directory, blob GC) is superseded. All payloads go to `artifacts` with no inline threshold.
- **ADR 002**: The safety-net dedup on seal (`bearing::seal`'s deduplication logic) is superseded. The slate enforces one observation per target at write time; there is nothing to deduplicate at seal.
