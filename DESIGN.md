# Design

How the vision maps to Rust types and the decisions behind them.

See [VISION.md](VISION.md) for the concepts and the why.

## Type Map

| Concept | Type | Module |
|---------|------|--------|
| Voyage | `Voyage`, `VoyageKind`, `VoyageStatus` | `model::voyage` |
| Bearing | `Bearing` | `model::bearing` |
| Observation | `Observation` | `model::source` |
| Mark | `Mark` | `model::source` |
| Sighting | `Sighting` | `model::source` |
| Reading | `Reading`, `ReadingAttempt`, `ReadingSource` | `model::reading` |
| Action | `Action`, `Act` | `model::action` |
| Logbook entry | `LogbookEntry` | `model` |

## Voyages

```rust
Voyage {
    id: Uuid,
    kind: VoyageKind,       // OpenWaters | ResolveIssue
    intent: String,
    created_at: Timestamp,
    status: VoyageStatus,   // Active | Completed { completed_at, summary }
}
```

A voyage is created, worked, and completed.
The `intent` is freeform — it's whatever the human wrote when starting the voyage.
`VoyageKind` frames the first bearing but doesn't constrain the voyage after that.

## Bearings

```rust
Bearing {
    observations: Vec<Observation>,
    reading: Reading,
    taken_at: Timestamp,
}
```

A bearing seals observations with a reading.
Observations you took but didn't find useful are simply not included.
Bearings have no ID — they're identified by position in the logbook stream.

## Observations

```rust
Observation {
    mark: Mark,
    sighting: Sighting,
    observed_at: Timestamp,
}
```

Self-contained: what you looked at, what you saw, when.
The mark carries enough structure that the logbook tells a story without replaying the sighting.

## Marks

A mark is what you pointed the spyglass at.
Each variant describes a domain of observable reality with enough detail to reconstruct *what was observed* at a glance.

The principle: **marks + readings should tell the logbook story without replaying sightings.**
When scanning bearings, you should know what was looked at and what was concluded.
The sighting is the raw evidence — useful for the current session, but not needed for the narrative.

### Implemented

```rust
Mark::Files {
    scope: Vec<PathBuf>,    // directories to survey
    focus: Vec<PathBuf>,    // files to inspect
}

Mark::RustProject {
    root: PathBuf,          // project root, walks tree + reads source files
}
```

`Files` separates scope (survey) from focus (inspect).
`RustProject` is a composite — it surveys the tree and inspects all source files unconditionally.
Both use flat vectors, not the scope-to-focus map described in VISION.md.
The flat model is simpler and hasn't created friction yet.

### Planned

**GitHub** — PRs, issues, checks, comments.
Needs enough mark structure to distinguish "looked at PR #42 metadata" from "read the inline review comments on PR #42."
Design of scope/focus semantics for GitHub is the prerequisite to building the observation logic.

Candidate structure:

```rust
Mark::GitHub {
    target: GitHubTarget,   // what kind of thing
    focus: Vec<GitHubFocus>, // what specifically to inspect
}
```

Where `GitHubTarget` might be `PullRequest(u64)`, `Issue(u64)`, `Repository`, etc.
And `GitHubFocus` might be `Diff`, `InlineComments`, `Checks`, `Comments`, `Approvals`, etc.

This is a sketch — the actual types need to be worked through when building the GitHub mark.

**Context** — human-provided information with no system-observable source.
Offline conversations, decisions made outside the tool, background knowledge.
No sighting to fetch — just a mark that describes what the context is about, and a reading the human attaches.

## Sightings

What was seen when observing a mark.
One `Sighting` variant per mark domain.

```rust
Sighting::Files {
    survey: Vec<DirectorySurvey>,       // directory listings
    inspections: Vec<FileInspection>,   // file contents
}
```

`DirectorySurvey` contains a path and a list of `DirectoryEntry` (name, is_dir, size).
`FileInspection` contains a path and `FileContent` (Text, Binary, or Error).

`RustProject` reuses `Sighting::Files` — same structure, different mark that produced it.

No `Sighting::GitHub` exists yet.

## Readings

```rust
Reading {
    text: String,
    history: Vec<ReadingAttempt>,
}

ReadingAttempt {
    text: String,
    source: ReadingSource,          // Agent | User
    challenged_with: Option<String>,
}
```

The reading is the interpretation of what was observed.
`text` is the accepted reading.
`history` tracks prior attempts that were challenged — alignment gaps, not failures.

`ReadingSource` records who authored each attempt (agent or human).

## Actions

```rust
Action {
    id: Uuid,
    identity: String,       // who acted (e.g. "dyreby", "john-agent")
    act: Act,
    performed_at: Timestamp,
}
```

Actions are things that changed the world.
Only successful operations are recorded — the logbook captures what happened, not what was attempted.

### Act types

Grouped by target, not by verb.

```rust
Act::Pushed { branch, sha }

Act::PullRequest { number, act: PullRequestAct }
// PullRequestAct: Created | Merged | Commented | Replied | RequestedReview

Act::Issue { number, act: IssueAct }
// IssueAct: Created | Closed | Commented
```

`Replied` is distinct from `Commented` — "I addressed inline feedback" is a meaningful signal in the logbook.

Identity selects which GitHub account to use.
Each identity has its own `gh` config directory under `~/.helm/gh-config/<identity>/`.

## The Logbook

```rust
LogbookEntry::Bearing(Bearing)
LogbookEntry::Action(Action)
```

Append-only JSONL.
One file per voyage at `~/.helm/voyages/<uuid>/logbook.jsonl`.
Tagged enum so each line is self-describing.

Voyages also have `voyage.json` for metadata.

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
See #49 for the planned separation of observations into prunable artifacts.

## The Agent Contract

The agent is stateless.
Every call receives explicit context and returns a structured result.

- **Take Bearing**: receives bearing history → produces a reading.
- **Correct Reading**: receives bearing history + feedback → produces a revised reading.
- **Correct Course**: receives current bearing + history → returns new marks to observe, an action plan, or abort.

The agent never executes tools.
The agent never sees raw sightings from prior bearings.
The agent never expands scope without human approval.

## CLI

Helm is CLI-only. The interface:

```
helm voyage new <intent> [--kind open-waters|resolve-issue]
helm voyage list
helm voyage log <id>
helm voyage complete <id> [--summary <text>]

helm observe files [--scope <dir>...] [--focus <file>...]
helm observe rust-project <path>

helm record <voyage-id> --reading <text> [--observation <file>...]

helm act <voyage-id> --as <identity> <act-subcommand>
```

`helm observe` outputs JSON to stdout or `--out <file>`.
`helm record` reads observations from `--observation` files or stdin.
`helm act` executes the operation and records it in the logbook.

## Open Questions

- **Scope/focus modeling**: flat vectors work for Files.
  Will GitHub need the map structure VISION describes, or can it stay flat?
- **Observation storage** (#49): bearings currently inline observations.
  Planned: store observations as separate artifacts, prunable without breaking the logbook narrative.
- **Context mark**: structure TBD.
  Minimum viable: a description string and a reading. May need more structure as usage reveals patterns.
- **Search as a mark kind**: peer to other marks, or cross-cutting?
  VISION raises this. No pressure to resolve it yet.
