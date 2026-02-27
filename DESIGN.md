# Design

The current design for Helm. Serves [VISION.md](VISION.md). The vision serves [CHARTER.md](CHARTER.md).

If this design changes, that's growth — the vision held, and we learned something.

This document is where design decisions are proposed, discussed, and recorded.

## Terminology

The nautical metaphor is load-bearing. These terms are used consistently across code, CLI, and docs.

| Term | What it means | Rust type |
|------|---------------|-----------|
| **Voyage** | A unit of work with intent, logbook, and outcome | `Voyage` |
| **Logbook** | Append-only record of a voyage's bearings and actions | `Vec<LogbookEntry>` |
| **Bearing** | What was looked at and what it meant — mark + reading | `Bearing` |
| **Observation** | What was looked at and what was seen — mark + sighting | `Observation` |
| **Mark** | What you pointed the spyglass at — a domain of observable reality | `Mark` |
| **Sighting** | The raw data returned when observing a mark | `Sighting` |
| **Reading** | Short interpretation of what was observed — the logbook's narrative voice | `Reading` |
| **Action** | An operation Helm performed | `Action` |
| **Action kind** | The specific thing that was done (push, create PR, comment) | `ActionKind` |

### Canonical Verbs

Verbs pair with nouns consistently across code, CLI, and docs.

| Noun | Verb | Example |
|------|------|---------|
| **Mark** | observe | "Observe a mark" — look at it, capture a sighting |
| **Bearing** | take | "Take a bearing" — interpret what was observed, write it to the logbook |
| **Action** | perform | "Perform an action" — do something in the world, the logbook records it |
| **Voyage** | start / complete | "Start a voyage" / "Complete a voyage" |

The logbook **records** — that's its job, not the caller's verb. You observe, take bearings, and perform actions. The logbook captures all of it.

Marks + readings tell the logbook story. Sightings are the raw evidence — useful during the session, not needed for the narrative.

### Bearing vs. Observation

A bearing and an observation both start from a mark, but they capture different things.

- **Bearing** = marks + reading. What you looked at and what you made of it. Lightweight, always in the logbook.
- **Observation** = mark + sighting. What you looked at and the raw data you saw. Heavy, stored separately, prunable.

A bearing can reference multiple marks — you looked at several things and formed one reading.
An observation is always a single look — one mark and what came back.

Deleting an observation doesn't break the logbook's story — you still know what was looked at and what was concluded.
The sighting is evidence; the reading is interpretation. Both reference the same marks, but they're decoupled records.

## Example Flow: Resolving an Issue

A complete voyage, from start to finish.

### 1. Start the voyage

```bash
$ helm voyage new --as john-agent --kind resolve-issue "Resolve #42: fix widget crash"
a3b0fc12-...
```

Creates:
```
~/.helm/voyages/a3b0fc12-.../
  voyage.json    # { kind: ResolveIssue, intent: "Resolve #42: ...", status: Active }
```

### 2. Observe the world

```bash
$ helm --voyage a3b observe rust-project . --out obs.json
Observation written to obs.json
```

Produces an `Observation`:
```
{
  mark: Mark::RustProject { root: "." },
  sighting: Sighting::RustProject { listings: [...], contents: [...] },
  observed_at: "2026-02-26T17:00:00Z"
}
```

The mark says *what* was looked at (a Rust project at `.`).
The sighting contains the full directory tree and documentation file contents.

Source code is not read. This is deliberate — `RustProject` is for orientation.
It answers "what is this project and how is it structured?" not "what does the code do?"
Once you know which files matter, use `Mark::FileContents` with targeted paths to read them.
Orient first, then target. This two-step pattern — broad observation to get your bearings,
then targeted reads where it matters — is central to how Helm works.

### 3. Take a bearing

```bash
$ helm --voyage a3b bearing --reading "Widget module has a null check missing in init(). Test coverage exists but doesn't hit this path." --observation obs.json
Bearing taken for voyage a3b0fc12
Reading: Widget module has a null check missing in init()...
```

Appends a `LogbookEntry::Bearing` to `logbook.jsonl`:
```
{
  marks: [RustProject { root: "." }],
  reading: { text: "Widget module has a null check...", history: [] },
  taken_at: "2026-02-26T17:01:00Z"
}
```

The observation (mark + sighting) is stored separately as a prunable artifact.
The bearing (marks + reading) stays in the logbook — lightweight, always available.
Scanning the logbook later, the bearing reads: *"Looked at the Rust project. Widget module has a null check missing."*

### 4. Do the work, then act

```bash
$ helm --voyage a3b action commit --message "Fix null check in widget init"
Action performed for voyage a3b0fc12
  committed (abc1234)

$ helm --voyage a3b action push --branch fix-widget
Action performed for voyage a3b0fc12
  pushed to fix-widget (abc1234)

$ helm --voyage a3b action create-pull-request --branch fix-widget --title "Fix widget crash" --reviewer dyreby
Action performed for voyage a3b0fc12
  created PR #45
```

Each action performs the operation and records it in the logbook. The logbook now has three `LogbookEntry::Action` entries.

### 5. Complete the voyage

```bash
$ helm --voyage a3b complete --summary "Fixed null check in widget init. PR #45 merged."
Voyage a3b0fc12 completed
Summary: Fixed null check in widget init. PR #45 merged.
```

### The logbook tells the story

```bash
$ helm --voyage a3b log
Voyage: Resolve #42: fix widget crash
Identity: john-agent
Created: 2026-02-26T17:00:00Z
Status: completed (2026-02-26T17:30:00Z)
Summary: Fixed null check in widget init. PR #45 merged.

── Bearing 1 ── 2026-02-26T17:01:00Z
  Mark: RustProject @ .
  Reading: Widget module has a null check missing in init().

── Action 2 ── 2026-02-26T17:14:00Z
  committed (abc1234)

── Action 3 ── 2026-02-26T17:15:00Z
  pushed to fix-widget (abc1234)

── Action 4 ── 2026-02-26T17:16:00Z
  created PR #45
```

Marks, readings, actions. The voyage's story, without replaying raw sightings.

## Types

### Mark

The central enum. Each variant is a domain of observable reality.
Adding a new source kind means adding a variant here and implementing its observation logic.

The mark carries enough structure that scanning the logbook tells a story —
you know *what* was looked at without replaying the sighting.

```rust
/// What you pointed the spyglass at.
///
/// Each variant describes a domain with enough detail to reconstruct
/// what was observed at a glance. The mark is the logbook's label;
/// the sighting is the raw data behind it.
enum Mark {
    /// Read specific files.
    ///
    /// The simplest filesystem mark.
    /// Returns the content of each file (text, binary, or error).
    FileContents { paths: Vec<PathBuf> },

    /// Recursive directory walk with filtering.
    ///
    /// Respects `.gitignore` by default.
    /// `skip` names directories to skip at any depth (e.g. "target", "node_modules").
    /// `max_depth` limits recursion depth (`None` = unlimited).
    DirectoryTree {
        root: PathBuf,
        skip: Vec<String>,
        max_depth: Option<u32>,
    },

    /// A Rust project rooted at a directory.
    ///
    /// Walks the project tree (respects `.gitignore`, skips `target/`).
    /// Lists the full directory tree with metadata.
    /// Reads documentation files only — README, VISION, CONTRIBUTING,
    /// agent instructions, etc. Source code is not read.
    ///
    /// This is an orientation mark. Use `FileContents` with targeted paths
    /// to read specific source files on subsequent observations.
    RustProject { root: PathBuf },

    /// A GitHub pull request.
    ///
    /// Focus controls depth: summary for metadata,
    /// diff/comments/reviews/checks/files for details.
    /// Defaults to summary when no focus is specified.
    GitHubPullRequest {
        number: u64,
        focus: Vec<PullRequestFocus>,   // Summary, Files, Checks, Diff, Comments, Reviews
    },

    /// A GitHub issue.
    ///
    /// Focus controls depth: summary for metadata, comments for discussion.
    /// Defaults to summary when no focus is specified.
    GitHubIssue {
        number: u64,
        focus: Vec<IssueFocus>,         // Summary, Comments
    },

    /// A GitHub repository.
    ///
    /// Lists open issues, pull requests, or both.
    GitHubRepository {
        focus: Vec<RepositoryFocus>,    // Issues, PullRequests
    },

    // ── Planned ──

    // Human-provided context with no system-observable source.
    //
    // Offline conversations, decisions made outside the tool, background knowledge.
    // No sighting to fetch — just a mark that describes what the context is about,
    // and a reading the human attaches.
    //
    // Structure TBD. Minimum viable: a description string.
    // Context {
    //     description: String,
    // },
}
```

Each filesystem mark describes one thing you pointed the spyglass at:
`FileContents` reads files, `DirectoryTree` shows structure.
`RustProject` is a domain-aware composite that uses `DirectoryTree` + `FileContents` internally.

### Sighting

Mirrors `Mark`. One variant per domain, containing the raw data from observation.

```rust
/// What was seen when observing a mark.
///
/// Each variant corresponds to a Mark variant.
/// The sighting is the heavy payload — full file contents,
/// directory trees, API responses. Stored separately from
/// the bearing and prunable.
enum Sighting {
    /// Contents of specific files.
    FileContents { contents: Vec<FileContents> },

    /// Recursive directory tree.
    DirectoryTree { listings: Vec<DirectoryListing> },

    /// Rust project structure and documentation.
    RustProject {
        listings: Vec<DirectoryListing>,
        contents: Vec<FileContents>,
    },

    /// Results from observing a GitHub pull request.
    /// Boxed to keep variant sizes balanced.
    GitHubPullRequest(Box<PullRequestSighting>),

    /// Results from observing a GitHub issue.
    GitHubIssue(Box<IssueSighting>),

    /// Results from observing a GitHub repository.
    GitHubRepository(Box<RepositorySighting>),
}

/// A directory listing: what's at this path.
struct DirectoryListing {
    path: PathBuf,
    entries: Vec<DirectoryEntry>,
}

/// A single entry in a directory listing.
struct DirectoryEntry {
    name: String,
    is_dir: bool,
    size_bytes: Option<u64>,
}

/// A file and what was in it.
struct FileContents {
    path: PathBuf,
    content: FileContent,
}

/// What was found when reading a file.
///
/// No file-type field (Rust, Markdown, TOML, etc.) — the file extension
/// is in the `path` and the consumer knows what to do with the content.
/// Adding a kind enum would duplicate derivable information and create
/// a maintenance surface for every new file type encountered.
///
/// If structured parsing is ever needed (Rust AST, Markdown sections),
/// that's a different sighting type, not a field here.
enum FileContent {
    /// UTF-8 text content.
    Text { content: String },

    /// File was not valid UTF-8. Size recorded for reference.
    Binary { size_bytes: u64 },

    /// File could not be read.
    Error { message: String },
}
```

Each mark gets its own sighting variant — a sighting contains exactly what its mark produces.
`DirectoryListing` and `DirectoryTree` both use `Vec<DirectoryListing>` (the struct);
the flat list of per-directory listings encodes the tree through paths.

A `RustProject` observation returns both structure and documentation:

```
Sighting::RustProject {
    listings: [
        DirectoryListing { path: ".",    entries: [src/, Cargo.toml, README.md, ...] },
        DirectoryListing { path: "./src", entries: [main.rs, model.rs, ...] },
    ],
    contents: [
        FileContents { path: "README.md",       content: Text { "# Helm\n..." } },
        FileContents { path: "CONTRIBUTING.md",  content: Text { "..." } },
    ],
}
```

`RustProject` walks the tree, so `listings` covers every directory — each file appears in its parent's listing so you know it exists and how big it is.
`contents` has only documentation. Source files like `main.rs` show up in listings but aren't read.

### Bearing

```rust
/// An immutable record: what was observed, and what it means.
///
/// A bearing can reference multiple marks — you looked at several things
/// and formed one reading. The bearing is the logbook's narrative unit.
///
/// Identified by position in the logbook stream, not by ID.
struct Bearing {
    marks: Vec<Mark>,
    observation_refs: Vec<u64>,
    reading: Reading,
    taken_at: Timestamp,
}
```

Marks are extracted from observations at record time, so the bearing is
self-describing — you see what was looked at without resolving refs.
`observation_refs` are voyage-scoped integer IDs pointing to files in `observations/`.

### Observation

```rust
/// A self-contained observation: one mark, one sighting, timestamped.
///
/// The raw capture. Take as many as you want; most are glances
/// that get discarded. The ones worth keeping are stored as artifacts
/// alongside the bearing, but separate from it.
///
/// Pruning observations doesn't break the logbook — the bearing
/// still has the marks and the reading.
struct Observation {
    mark: Mark,
    sighting: Sighting,
    observed_at: Timestamp,
}
```

### Reading

```rust
/// A short, plain-text interpretation of the world's state.
///
/// The reading is the logbook's narrative voice — what you concluded
/// from what you observed. Tracks the accepted text and the history
/// of attempts that were challenged along the way.
///
/// The challenge history captures alignment gaps, not failures.
struct Reading {
    /// The accepted reading text.
    text: String,

    /// Prior attempts that were challenged before arriving at the accepted text.
    history: Vec<ReadingAttempt>,
}

/// A single attempt at stating a reading, possibly challenged.
struct ReadingAttempt {
    /// The reading text that was proposed.
    text: String,

    /// Who produced this text.
    source: ReadingSource,

    /// Feedback that caused this attempt to be rejected.
    /// Present on challenged attempts, absent on the final accepted one.
    challenged_with: Option<String>,
}

/// Who authored a reading.
enum ReadingSource {
    /// Generated by the LLM.
    Agent,

    /// Written or edited by the user.
    User,
}
```

### Action

```rust
/// A single, immutable record of an operation Helm performed.
///
/// Only successful operations are recorded — the logbook captures
/// what happened, not what was attempted.
/// Identity is on the voyage, not the action.
struct Action {
    id: Uuid,

    /// What was done.
    kind: ActionKind,

    performed_at: Timestamp,
}

/// What was done. Grouped by target, not by verb.
///
/// This grouping is deliberate: "what happened to PR #42" is a more
/// natural question than "what was commented on." The target is the
/// primary key; the verb is secondary.
enum ActionKind {
    /// Committed changes locally.
    Commit { sha: String },

    /// Pushed commits to a branch.
    Push { branch: String, sha: String },

    /// An action on a pull request.
    PullRequest { number: u64, action: PullRequestAction },

    /// An action on an issue.
    Issue { number: u64, action: IssueAction },
}

/// Things you can do to a pull request.
enum PullRequestAction {
    Create,
    Merge,
    Comment,

    /// Replied to an inline review comment.
    /// Distinct from Comment because "I addressed feedback"
    /// is a meaningful signal when reading the logbook.
    Reply,

    RequestedReview { reviewers: Vec<String> },
}

/// Things you can do to an issue.
enum IssueAction {
    Create,
    Close,
    Comment,
}
```

Identity is set on the voyage at creation.
Each identity has its own `gh` config directory under `~/.helm/gh-config/<identity>/`.
A default identity is configured in `~/.helm/config.toml` and used when `--as` is omitted.

### Logbook

```rust
/// A single entry in the logbook, serialized as one line of JSONL.
///
/// Tagged enum so each line is self-describing when read back.
/// The logbook is append-only — nothing is overwritten or dropped.
enum LogbookEntry {
    /// A bearing was taken.
    Bearing(Bearing),

    /// An action was performed.
    Action(Action),
}
```

One file per voyage at `~/.helm/voyages/<uuid>/logbook.jsonl`.

### Voyage

```rust
/// A unit of work with intent, logbook, and outcome.
struct Voyage {
    id: Uuid,

    /// The identity sailing this voyage (e.g. "john-agent", "dyreby").
    /// Set at creation, inherited by all commands.
    /// Can change via spelling (handoff to another identity).
    identity: String,

    kind: VoyageKind,
    intent: String,
    created_at: Timestamp,
    status: VoyageStatus,
}

/// The kind of voyage, which frames the first bearing.
enum VoyageKind {
    /// Unscoped, general-purpose voyage.
    OpenWaters,

    /// Resolve a GitHub issue.
    ResolveIssue,
}

/// Where a voyage stands in its lifecycle.
enum VoyageStatus {
    Active,

    Completed {
        completed_at: Timestamp,
        summary: Option<String>,
    },
}
```

`VoyageKind` frames the first bearing but doesn't constrain the voyage after that.

## Source Kinds

Each mark describes a domain of observable reality — not a mechanism.
Commands are how Helm fetches data; marks describe what Helm is looking at.

- **FileContents** — read specific files. Implemented.
- **DirectoryTree** — recursive walk with `.gitignore`, skip patterns, depth limits. Implemented.
- **RustProject** — full project tree, documentation files. Domain-aware composite. Implemented.
- **GitHub** — PR/issue metadata, check summaries, diffs, comment bodies, inline review threads. Implemented as three marks: `GitHubPullRequest`, `GitHubIssue`, `GitHubRepository`, each with domain-specific focus items.
- **Context** — human-provided context with no system-observable source. Planned.
- **Web** — status, headers, response bodies. Future.

Web-based kinds graduate to their own domain when their observation semantics are rich enough.

## The Agent Contract

The agent is stateless. Every call receives explicit context and returns a structured result. No ongoing session. No hidden memory.

| Phase | Input | Output |
|-------|-------|--------|
| **Take Bearing** | Bearing history (observations + readings) | A reading |
| **Correct Reading** | Bearing history + human feedback | A revised reading |
| **Correct Course** | Current bearing + history + constraints | New marks to observe, an action plan, or abort |

Structural constraints — not instructions:
- The agent never executes tools.
- The agent never sees raw sightings from prior bearings.
- The agent never expands scope without human approval.

A reading describes; it never prescribes.
If it feels wrong, the human challenges it and the agent re-generates.

## Storage

```
~/.helm/
  config.toml           # default-identity and future settings
  voyages/
    <uuid>/
      voyage.json       # Voyage metadata
      logbook.jsonl     # Append-only logbook entries
      observations/     # Prunable observation artifacts
        1.json          # Observation referenced by bearing
        2.json
  gh-config/
    <identity>/         # Per-identity gh auth
```

Observations are stored as separate JSON files, one per observation.
IDs are linear integers scoped to the voyage.
Bearings reference observations by ID — deleting an observation file
doesn't break the logbook's narrative.
Pruning is manual: delete files from `observations/`.

## CLI

Commands split into two groups: voyage lifecycle (no `--voyage` needed)
and voyage-scoped operations (require `--voyage`).

Identity is set at voyage creation and inherited by all commands.
Helm is for voyages only — no voyage, no helm.

```
helm voyage new --as <identity> <intent> [--kind open-waters|resolve-issue]
helm voyage list

helm --voyage <id> observe file-contents --read <file>...
helm --voyage <id> observe directory-tree <root> [--skip <name>...] [--max-depth <n>]
helm --voyage <id> observe rust-project <path>
helm --voyage <id> observe github-pr <number> [--focus summary|files|checks|diff|comments|reviews]
helm --voyage <id> observe github-issue <number> [--focus summary|comments]
helm --voyage <id> observe github-repo [--focus issues|pull-requests]

helm --voyage <id> bearing --reading <text> [--observation <file>...]

helm --voyage <id> action <action-subcommand>

helm --voyage <id> complete [--summary <text>]
helm --voyage <id> log
```

`--voyage` takes a full UUID or unambiguous prefix (e.g. `a3b`).
`helm observe` outputs JSON to stdout or `--out <file>`.
`helm bearing` reads observations from `--observation` files or stdin.
`helm action` performs the operation and records it in the logbook.
Identity for `act` comes from the voyage — no per-command `--as`.

## Open Questions

- **Context mark**: structure TBD.
  Minimum viable: a description string and a reading.

