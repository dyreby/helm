# Design

The current design for Helm. Serves [VISION.md](VISION.md). The vision serves [CHARTER.md](CHARTER.md).

If this design changes, that's growth — the vision held, and we learned something.

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

A bearing and an observation both start from a mark, but they capture different things.
The bearing records marks + reading: what you looked at and what you made of it. Lightweight, always in the logbook.
The observation records mark + sighting: what you looked at and the raw data you saw. Heavy, stored separately, prunable.

Deleting an observation doesn't break the logbook's story — you still know what was looked at and what was concluded.
The sighting is evidence; the reading is interpretation. Both reference the same marks, but they're decoupled records.
| **Action** | Something that changed the world, recorded after the fact | `Action` |
| **Act** | The specific thing that was done (push, create PR, comment) | `Act` |
| **Scope** | What to survey — the broad view (directories, PRs) | Mark fields |
| **Focus** | What to inspect — the deep view (specific files, diffs) | Mark fields |

Marks + readings tell the logbook story. Sightings are the raw evidence — useful during the session, not needed for the narrative.

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
  sighting: Sighting::Files { survey: [...], inspections: [...] },
  observed_at: "2026-02-26T17:00:00Z"
}
```

The mark says *what* was looked at (a Rust project at `.`).
The sighting contains the full directory tree (survey) and documentation file contents (inspection).

Source code is not read. This is deliberate — `RustProject` is for orientation.
It answers "what is this project and how is it structured?" not "what does the code do?"
Once you know which files matter, use `Mark::Files` with focused paths to read them.
Orient first, then focus. This two-step pattern — broad observation to get your bearings,
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

### Voyage

```rust
Voyage {
    id: Uuid,
    kind: VoyageKind,       // OpenWaters | ResolveIssue
    intent: String,
    created_at: Timestamp,
    status: VoyageStatus,   // Active | Completed { completed_at, summary }
}
```

`VoyageKind` frames the first bearing but doesn't constrain the voyage after that.

### Bearing

```rust
Bearing {
    marks: Vec<Mark>,
    reading: Reading,
    taken_at: Timestamp,
}
```

Records what was looked at (marks) and what it meant (reading).
A bearing can reference multiple marks — you looked at several things and formed one reading.
The bearing is the logbook's narrative unit — lightweight and self-contained.
Bearings have no ID — identified by position in the logbook stream.

> **Note:** The current implementation stores `observations: Vec<Observation>` in bearings,
> inlining the full sightings. The intended design separates them — bearings reference marks,
> observations are stored as separate prunable artifacts. See #49.

### Observation

```rust
Observation {
    mark: Mark,
    sighting: Sighting,
    observed_at: Timestamp,
}
```

The raw capture: one mark, one sighting.
An observation is always a single look — one mark and what came back.
Take as many as you want. Most are glances — quick looks that get discarded.
The ones worth keeping are stored as artifacts alongside the bearing, but separate from it.
Pruning observations doesn't break the logbook — the bearing still has the marks and the reading.

### Mark

Each variant describes a domain of observable reality.
Adding a new source kind means adding a `Mark` variant and implementing its observation logic.

```rust
Mark::Files {
    scope: Vec<PathBuf>,    // directories to survey
    focus: Vec<PathBuf>,    // files to inspect
}

Mark::RustProject {
    root: PathBuf,
}
```

`Files` separates scope (survey) from focus (inspect) as flat vectors.
`RustProject` is a composite — surveys the full tree and inspects documentation files (README, VISION, CONTRIBUTING, agent instructions, etc.). Source code is left for targeted `Mark::Files` queries.

#### Planned: GitHub

```rust
Mark::GitHub {
    target: GitHubTarget,       // PullRequest(u64) | Issue(u64) | Repository
    focus: Vec<GitHubFocus>,    // Diff | InlineComments | Checks | Comments | Approvals
}
```

Enough structure to distinguish "looked at PR #42 metadata" from "read the inline review comments on PR #42."
This is a sketch — the actual types will be worked through when building the GitHub mark.

#### Planned: Context

Human-provided information with no system-observable source.
Offline conversations, decisions made outside the tool, background knowledge.
No sighting to fetch — just a mark that describes what the context is about, and a reading the human attaches.

### Sighting

One variant per mark domain. The raw data returned by observation.

```rust
Sighting::Files {
    survey: Vec<DirectorySurvey>,       // directory listings
    inspections: Vec<FileInspection>,   // file contents
}
```

Supporting types:
- `DirectorySurvey` — path + list of `DirectoryEntry` (name, is_dir, size)
- `FileInspection` — path + `FileContent` (Text, Binary, or Error)

`RustProject` reuses `Sighting::Files`.

### Reading

```rust
Reading {
    text: String,                       // the accepted interpretation
    history: Vec<ReadingAttempt>,       // prior attempts that were challenged
}

ReadingAttempt {
    text: String,
    source: ReadingSource,              // Agent | User
    challenged_with: Option<String>,    // feedback that caused rejection
}
```

The challenge history captures alignment gaps in the collaboration, not failures.

### Action

```rust
Action {
    id: Uuid,
    identity: String,       // who acted (e.g. "dyreby", "john-agent")
    act: Act,
    performed_at: Timestamp,
}
```

Only successful operations are recorded.
The logbook captures what happened, not what was attempted.

Act types are grouped by target, not by verb:

```rust
Act::Pushed { branch, sha }
Act::PullRequest { number, act: PullRequestAct }
Act::Issue { number, act: IssueAct }
```

`PullRequestAct`: Created, Merged, Commented, Replied, RequestedReview.
`IssueAct`: Created, Closed, Commented.

`Replied` is distinct from `Commented` — "I addressed inline feedback" is a meaningful signal when reading the logbook.

### Logbook

```rust
LogbookEntry::Bearing(Bearing)
LogbookEntry::Action(Action)
```

Append-only JSONL. Tagged enum so each line is self-describing.

## Source Kinds

Each mark describes a domain of observable reality — not a mechanism.
Commands are how Helm fetches data; marks describe what Helm is looking at.

| Kind | Survey (broad scan) | Inspect (deep dive) | Status |
|------|--------------------|--------------------|--------|
| **Files** | Directory trees with metadata | File contents | Implemented |
| **RustProject** | Full project tree | Documentation files | Implemented |
| **GitHub** | PR/issue metadata, check summaries | Diffs, comment bodies, threads | Planned |
| **Context** | — | — | Planned |
| **Web** | Status and headers | Response bodies | Future |
| **Search** | Hit lists with locations | Matches with context | Future |

Web-based kinds graduate to their own domain when their scope/focus semantics are rich enough.
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
  Will GitHub need a richer structure, or can it stay flat?
- **Observation storage** (#49): bearings currently inline observations.
  Planned: store separately, prunable without breaking the narrative.
- **Context mark**: structure TBD.
  Minimum viable: a description string and a reading.
- **Search**: peer kind or cross-cutting layer?
