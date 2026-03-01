# Design

The current design for Helm. Serves [VISION.md](VISION.md). The vision serves [CHARTER.md](CHARTER.md).

If this design changes, that's growth — the vision held, and we learned something.

Design decisions are recorded as ADRs in [docs/adr/](docs/adr/).

## Terminology

The nautical metaphor is load-bearing. These terms are used consistently across code, CLI, and docs.

| Term | What it means |
|------|---------------|
| **Voyage** | A unit of work with a logbook |
| **Logbook** | Append-only record of a voyage's entries (steer actions, logged states) |
| **Observation** | What you looked at (`Observe` variant) + what came back (payload) + timestamp |
| **Bearing** | Sealed observations + summary. Recorded in each log entry on steer/log |
| **Slate** | Observations accumulating between steer/log commands |
| **Artifact** | Content-addressed, zstd-compressed payload. Stored once per voyage database; referenced by hash from slate and sealed bearings |

### Commands

| Command | What it does | Writes to logbook? |
|---------|-------------|-------------------|
| **`helm observe`** | Gather observations into the slate | No |
| **`helm steer`** | Mutate collaborative state | Yes |
| **`helm log`** | Record state without mutation | Yes |

Only `steer` and `log` write to the logbook. That's the invariant.

### Canonical Verbs

| Noun | Verb | Example |
|------|------|---------|
| **Observation** | observe | "Observe an issue" — look at it, capture what came back |
| **Bearing** | seal | "Seal a bearing" — seal the slate into a bearing at decision time |
| **Voyage** | start / end | "Start a voyage" / "End a voyage" |

The logbook **records** — that's its job, not the caller's verb. You observe, steer, and log. The logbook captures what happened.

## The Three Commands

### `helm observe`

Gather observations into the slate. Never writes to the logbook. Cheap, frequent, ephemeral.

An observation has three parts:

- **target** — what you looked at (`Observe` variant)
- **payload** — what came back (stored as a content-addressed artifact)
- **timestamp** — when the observation was made

The `Observe` enum is the extension surface for new observation types. Add a variant to teach helm to look at something new.

### `helm steer <intent>`

Perform an intent-based domain action that mutates collaborative state. One invocation = one logbook entry.

What happens atomically:

1. Seal the slate into a bearing
2. Perform the action
3. Record one logbook entry
4. Clear the slate

A single steer may perform multiple API calls internally (e.g., post a comment + add a label), but it logs as one semantic action.

Steer subcommands are the extension surface for new capabilities. Each is a deterministic flow with a known shape. The stable contract is: seal, perform, record, clear.

Initial steer subcommands:

- `comment` — comment on an issue or PR
- `create-issue` — create an issue
- `edit-issue` — update issue title/body
- `close-issue` — close an issue
- `create-pr` — create a pull request
- `edit-pr` — update PR title/body
- `close-pr` — close a PR without merging
- `request-review` — request reviewers on a PR
- `reply-inline` — reply to an inline code review comment on a PR
- `merge-pr` — merge a PR

### `helm log`

Record a deliberate state without mutating collaborative state. Same seal-and-clear behavior as steer.

Use this when the voyage reaches a state worth recording but there's nothing to change in the world:

- Waiting for feedback
- Blocked on a decision
- Ready for the next step

## Collaborative State as the Boundary

Only state transitions that cross the collaborative boundary get logged as steer actions. Local operations (git commits, branch management, file edits) are implementation details.

Steer actions are typed by semantic intent, not by API call shape. The logbook records what happened (commented on issue, opened PR), not how (POST to /repos/.../comments).

GitHub is the current collaborative boundary. The model supports other boundaries in the future without design changes.

## Slate and Bearing

Observations accumulate in the slate between steer/log commands. When either is called, helm seals the slate into a bearing:

- Deduplication is enforced at write time — `INSERT OR REPLACE` on the slate means the same target observed twice leaves one entry, the newest payload wins.
- Seal copies the slate into `bearing_observations` and clears it, atomically in one transaction.
- No manual step. The invariant: any command that writes to the logbook seals and clears.

All payloads are stored as content-addressed artifacts — zstd-compressed and keyed by SHA-256 hash of the uncompressed JSON. The same payload observed twice stores one artifact. Deduplication is free.

## Example Flow: Advancing an Issue

A voyage from issue through PR to merge.

### 1. Start a voyage

Create a voyage with intent.

### 2. Observe

Observe the issue, the project structure, relevant source files.
Each observation lands in the slate. Nothing is logged yet.

### 3. Steer: comment with a plan

Steer to comment on the issue with a proposed plan.
Helm seals a bearing from the slate, posts the comment, records one logbook entry, clears the slate.

### 4. Log: waiting

Log a waiting state. Seals and clears, records the state. No collaborative state changes.

### 5. Observe feedback, steer: create a PR

Observe new comments on the issue. Steer to create a PR.

### 6. Steer: request review

Steer to request review on the PR.

### 7. Observe review, steer: reply to feedback

Observe the PR (inline review comments). Steer to reply inline.

### 8. Steer: merge

Steer to merge the PR.

### 9. End the voyage

End the voyage with a freeform status.

### The logbook tells the story

```
Voyage: Advance #42: fix widget crash

── Steer 1 ── comment on issue #42
── Log 2 ── waiting
── Steer 3 ── create PR #45
── Steer 4 ── request review on PR #45
── Steer 5 ── reply inline on PR #45
── Steer 6 ── merge PR #45
```

Each entry carries its bearing (the observations that informed it) and the identity of who steered.
The voyage's story, without implementation noise.

## Types

### Observe

The central enum. Each variant describes something helm can look at.
Adding a new observation type means adding a variant here and implementing its observation logic.

```rust
enum Observe {
    /// Read specific files.
    FileContents { paths: Vec<PathBuf> },

    /// Recursive directory walk with filtering.
    DirectoryTree {
        root: PathBuf,
        skip: Vec<String>,
        max_depth: Option<u32>,
    },

    /// A Rust project rooted at a directory.
    /// Walks the project tree, lists structure, reads documentation only.
    /// An orientation observation — use FileContents for targeted reads.
    RustProject { root: PathBuf },

    /// A GitHub issue.
    GitHubIssue {
        number: u64,
        focus: Vec<IssueFocus>,
    },

    /// A GitHub pull request.
    GitHubPullRequest {
        number: u64,
        focus: Vec<PullRequestFocus>,
    },

    /// A GitHub repository.
    GitHubRepository {
        focus: Vec<RepositoryFocus>,
    },
}
```

### Observation

```rust
/// A single observation: what was looked at and what came back.
struct Observation {
    /// What was looked at.
    target: Observe,

    /// What came back — stored as a content-addressed artifact.
    payload: Payload,

    /// When the observation was made.
    observed_at: Timestamp,
}
```

### Bearing

```rust
/// Orientation at the moment of decision.
///
/// Sealed from the slate when steer or log is called.
/// One bearing per log entry — many observations feed into
/// one understanding of where you are.
struct Bearing {
    /// The observations that informed this decision.
    observations: Vec<Observation>,

    /// Freeform interpretation of the current state.
    summary: String,
}
```

### Steer

```rust
/// Intent-based actions that mutate collaborative state.
///
/// Each variant is a steer subcommand — a deterministic flow
/// with a known shape. This enum grows as helm learns new
/// capabilities.
enum Steer {
    Comment { /* TBD */ },
    CreateIssue { /* TBD */ },
    EditIssue { /* TBD */ },
    CloseIssue { /* TBD */ },
    CreatePullRequest { /* TBD */ },
    EditPullRequest { /* TBD */ },
    ClosePullRequest { /* TBD */ },
    RequestReview { /* TBD */ },
    ReplyInline { /* TBD */ },
    MergePullRequest { /* TBD */ },
}
```

Variant fields are implementation detail — defined when each subcommand is built.

### Voyage

```rust
/// A unit of work with a logbook.
struct Voyage {
    id: Uuid,
    intent: String,
    created_at: Timestamp,
    status: VoyageStatus,
}

enum VoyageStatus {
    Active,
    Ended {
        ended_at: Timestamp,
        status: Option<String>,
    },
}
```

## Storage

One `SQLite` file per voyage:

```
~/.helm/
  voyages/
    <uuid>.sqlite
```

The schema is versioned via `PRAGMA user_version`. Each voyage database has six tables:

- **`voyage`** — voyage metadata (id, intent, created\_at, status).
- **`artifacts`** — zstd-compressed payloads keyed by SHA-256 hash.
- **`artifact_derivations`** — links a reduced artifact to its summary (for future `helm artifact reduce`).
- **`slate`** — current observations, keyed by target. Set semantics enforced by the database.
- **`logbook`** — one row per steer or log entry, with identity, role, method, summary, and action.
- **`bearing_observations`** — the slate snapshot at the time of each logbook entry.

Foreign key enforcement (`PRAGMA foreign_keys = ON`) is set on every connection.

## CLI

Helm's CLI has two groups:

- **Voyage lifecycle**: create, end, list voyages.
- **Voyage-scoped**: observe, steer, log — require a voyage context.

Specific flags and arguments are implementation detail, defined when each subcommand is built.

## Identity

Identity is recorded per log entry, not per voyage. Multiple agents or people can steer the same voyage — the logbook records who did what.

Identity is required explicitly via `--as` on every `steer` and `log` invocation. No defaults, no config file.

## Open Questions

These are implementation questions that don't affect the design decisions captured in [ADR 001](docs/adr/001-observe-steer-log.md).

- **Bearing summary**: who writes it? Always the caller? Auto-generated? Optional?
