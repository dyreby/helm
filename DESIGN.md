# Design

Implementation of the [vision](VISION.md).

## CLI Grammar

```
helm voyage new <intent> [--kind <kind>]
helm voyage list
helm voyage log <id>
helm voyage complete <id> [--summary <text>]

helm observe <source-kind> [options] [--out <file>]
helm record <voyage-id> <position> [--observation <file>...]

helm act <voyage-id> --as <identity> <act> [options]
```

Voyage IDs accept unambiguous prefixes (`a3b` instead of the full UUID).

## Concepts

| Name | Role |
|------|------|
| **Voyage** | A unit of work with intent, logbook, and outcome |
| **Logbook** | Append-only sequence of bearings and actions |
| **Bearing** | Immutable record: observations + position |
| **Observation** | Self-contained: subject + sighting + timestamp |
| **Subject** | What you pointed the spyglass at (a source kind + scope/focus) |
| **Sighting** | What you saw |
| **Position** | Short statement of world state |
| **Action** | Immutable record of something that changed the world |
| **Act** | What was done (push, create PR, merge, etc.) |

## Voyage Kinds

- **Open Waters** — general-purpose, no prescribed framing.
- **Resolve Issue** — framed around understanding and closing a specific issue.

## Source Kinds

- **Files** — filesystem structure and content. Scope: directories to survey. Focus: specific files to inspect.
- **Rust Project** — a Rust project rooted at a directory. Walks the tree, respects `.gitignore`, produces structure and source contents.

GitHub, web, and search are planned additions.

## Actions and Identity

Actions execute real commands (git, `gh`) and record the result.
Identity (`--as`) selects which GitHub account to use.
Each identity has its own auth config under `~/.helm/gh-config/`.

### Act Types

**Git:**
- `push` — push commits to a branch.

**Pull Requests:**
- `create-pull-request` — open a new PR.
- `merge-pull-request` — squash merge and delete branch.
- `comment-on-pull-request` — leave a comment.
- `reply-on-pull-request` — reply to an inline review comment.
- `request-review` — request review from users.

**Issues:**
- `create-issue` — open a new issue.
- `close-issue` — close an issue.
- `comment-on-issue` — leave a comment.

## Storage

Voyages and logbook entries are stored as JSON files under `~/.helm/`.
Logbook entries are JSONL — one entry per line, appended in order.

## Workflow Example

```
$ helm voyage new "Resolve #42: fix widget crash" --kind resolve-issue
a3b0fc12

$ helm observe rust-project . --out obs.json
$ helm record a3b "Null check missing in widget init path" --observation obs.json

$ helm act a3b --as john-agent push --branch fix-widget --message "Fix null check"
$ helm act a3b --as john-agent create-pull-request --branch fix-widget --title "Fix widget"
$ helm act a3b --as john-agent merge-pull-request 45

$ helm voyage complete a3b --summary "Fixed null check in widget init"
```
