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
| **Action** | Something that changed the world, recorded after the fact | `Action` |
| **Act** | The specific thing that was done (push, create PR, comment) | `Act` |

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
$ helm voyage new "Resolve #42: fix widget crash" --kind resolve-issue
a3b0fc12-...
```

Creates:
```
~/.helm/voyages/a3b0fc12-.../
  voyage.json    # { kind: ResolveIssue, intent: "Resolve #42: ...", status: Active }
```

### 2. Observe the world

```bash
$ helm observe rust-project . --out obs.json
Observation written to obs.json
```

Produces an `Observation`:
```
{
  mark: Mark::RustProject { root: "." },
  sighting: Sighting::Files { directories: [...], files: [...] },
  observed_at: "2026-02-26T17:00:00Z"
}
```

The mark says *what* was looked at (a Rust project at `.`).
The sighting contains the full directory tree and documentation file contents.

Source code is not read. This is deliberate — `RustProject` is for orientation.
It answers "what is this project and how is it structured?" not "what does the code do?"
Once you know which files matter, use `Mark::Files` with targeted paths to read them.
Orient first, then target. This two-step pattern — broad observation to get your bearings,
then targeted reads where it matters — is central to how Helm works.

### 3. Record a bearing

```bash
$ helm record a3b --reading "Widget module has a null check missing in init(). Test coverage exists but doesn't hit this path." --observation obs.json
Bearing recorded for voyage a3b0fc12
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
$ helm act a3b --as john-agent push --branch fix-widget --message "Fix null check in widget init"
Action recorded for voyage a3b0fc12
  as: john-agent
  pushed to fix-widget (abc1234)

$ helm act a3b --as john-agent create-pull-request --branch fix-widget --title "Fix widget crash" --reviewer dyreby
Action recorded for voyage a3b0fc12
  as: john-agent
  created PR #45
```

Each act executes the operation *and* records it. The logbook now has two `LogbookEntry::Action` entries.

### 5. Complete the voyage

```bash
$ helm voyage complete a3b --summary "Fixed null check in widget init. PR #45 merged."
Voyage a3b0fc12 completed
Summary: Fixed null check in widget init. PR #45 merged.
```

### The logbook tells the story

```bash
$ helm voyage log a3b
Voyage: Resolve #42: fix widget crash
Created: 2026-02-26T17:00:00Z
Status: completed (2026-02-26T17:30:00Z)
Summary: Fixed null check in widget init. PR #45 merged.

── Bearing 1 ── 2026-02-26T17:01:00Z
  Mark: RustProject @ .
  Reading: Widget module has a null check missing in init().

── Action 2 ── 2026-02-26T17:15:00Z
  as: john-agent
  pushed to fix-widget (abc1234)

── Action 3 ── 2026-02-26T17:16:00Z
  as: john-agent
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
    /// Filesystem structure and content.
    ///
    /// Dumb and literal: no recursion, no filtering, no domain knowledge.
    /// You tell it exactly what to list and read.
    /// Domain marks like `RustProject` are where the smarts live.
    ///
    /// `list`: directories to list immediate contents of.
    /// `read`: files to read.
    Files {
        list: Vec<PathBuf>,
        read: Vec<PathBuf>,
    },

    /// A Rust project rooted at a directory.
    ///
    /// Lists the full tree (respects .gitignore, skips target/).
    /// Reads documentation files only — README, VISION, CONTRIBUTING,
    /// agent instructions, etc. Source code is not read.
    ///
    /// This is an orientation mark. Use `Files` with targeted paths
    /// to read specific source files on subsequent bearings.
    RustProject {
        root: PathBuf,
    },

    // ── Planned ──

    /// GitHub: PRs, issues, checks, comments.
    ///
    /// Enough structure to distinguish "looked at PR #42 metadata"
    /// from "read the inline review comments on PR #42."
    ///
    /// Sketch — actual types to be worked through when building this mark.
    // GitHub {
    //     target: GitHubTarget,       // PullRequest(u64) | Issue(u64) | Repository
    //     focus: Vec<GitHubFocus>,    // Diff | InlineComments | Checks | Comments | Approvals
    // },

    /// Human-provided context with no system-observable source.
    ///
    /// Offline conversations, decisions made outside the tool, background knowledge.
    /// No sighting to fetch — just a mark that describes what the context is about,
    /// and a reading the human attaches.
    ///
    /// Structure TBD. Minimum viable: a description string.
    // Context {
    //     description: String,
    // },
}
```

`Files` separates list (directories) from read (files) as flat vectors.
`RustProject` is a composite that does both — lists the tree, reads docs.

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
    /// Results from observing a Files or RustProject mark.
    Files {
        /// Directory listings from listed paths.
        listings: Vec<DirectoryListing>,

        /// File contents from read paths.
        contents: Vec<FileContents>,
    },

    // ── Planned ──

    // GitHub { ... },
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

`RustProject` reuses `Sighting::Files` — same structure, different mark that produced it.
A `RustProject` observation returns both:

```
Sighting::Files {
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

`listings` has the full tree — every file appears here so you know it exists and how big it is.
`contents` has only documentation contents. Source files like `main.rs` show up in the listing but aren't read.

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
    reading: Reading,
    taken_at: Timestamp,
}
```

> **Note:** The current implementation stores `observations: Vec<Observation>`,
> inlining full sightings. The intended design separates them — bearings reference
> marks, observations are stored as separate prunable artifacts. See #49.

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
/// A single, immutable record of something that changed the world.
///
/// Only successful operations are recorded — the logbook captures
/// what happened, not what was attempted.
struct Action {
    id: Uuid,

    /// Which identity performed this action (e.g. "dyreby", "john-agent").
    identity: String,

    /// What was done.
    act: Act,

    performed_at: Timestamp,
}

/// What was done. Grouped by target, not by verb.
///
/// This grouping is deliberate: "what happened to PR #42" is a more
/// natural question than "what was commented on." The target is the
/// primary key; the verb is secondary.
enum Act {
    /// Pushed commits to a branch.
    Pushed { branch: String, sha: String },

    /// An action on a pull request.
    PullRequest { number: u64, act: PullRequestAct },

    /// An action on an issue.
    Issue { number: u64, act: IssueAct },
}

/// Things you can do to a pull request.
enum PullRequestAct {
    Created,
    Merged,
    Commented,

    /// Replied to an inline review comment.
    /// Distinct from Commented because "I addressed feedback"
    /// is a meaningful signal when reading the logbook.
    Replied,

    RequestedReview { reviewers: Vec<String> },
}

/// Things you can do to an issue.
enum IssueAct {
    Created,
    Closed,
    Commented,
}
```

Identity selects which GitHub account to use.
Each identity has its own `gh` config directory under `~/.helm/gh-config/<identity>/`.

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

| Kind | Structure | Content | Status |
|------|----------|---------|--------|
| **Files** | Directory listings with metadata | File contents | Implemented |
| **RustProject** | Full project tree | Documentation files | Implemented |
| **GitHub** | PR/issue metadata, check summaries | Diffs, comment bodies, threads | Planned |
| **Context** | — | — | Planned |
| **Web** | Status and headers | Response bodies | Future |
| **Search** | Hit lists with locations | Matches with context | Future |

Web-based kinds graduate to their own domain when their observation semantics are rich enough.
GitHub is the first domain that graduated.

Whether Search is a peer kind or something that layers on top of other kinds is unresolved.

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
  voyages/
    <uuid>/
      voyage.json       # Voyage metadata
      logbook.jsonl     # Append-only logbook entries
  gh-config/
    <identity>/         # Per-identity gh auth
```

Observations are currently inlined in bearings.
See #49 for the planned separation into prunable artifacts.

## CLI

```
helm voyage new <intent> [--kind open-waters|resolve-issue]
helm voyage list
helm voyage log <id>
helm voyage complete <id> [--summary <text>]

helm observe files [--list <dir>...] [--read <file>...]
helm observe rust-project <path>

helm record <voyage-id> --reading <text> [--observation <file>...]

helm act <voyage-id> --as <identity> <act-subcommand>
```

`helm observe` outputs JSON to stdout or `--out <file>`.
`helm record` reads observations from `--observation` files or stdin.
`helm act` executes the operation and records it in the logbook.

## Open Questions

- **List/read modeling**: flat vectors work for Files.
  Will GitHub need a richer structure, or can it stay flat?
- **Observation storage** (#49): bearings currently inline observations.
  Planned: store separately, prunable without breaking the narrative.
- **Context mark**: structure TBD.
  Minimum viable: a description string and a reading.
- **Search**: peer kind or cross-cutting layer?
