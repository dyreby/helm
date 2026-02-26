# Vision

Voyages give structure to observation, action, and recording.
Each voyage is a unit of work with an intent, an append-only logbook, and an outcome.

## Why

I'm working on a [collaboration framework](https://github.com/dyreby/collaboration-framework)
for narrowing the gap between intent and understanding.
The concepts work. The philosophy holds.
But the mechanism I was using — prompt extensions injected into an agent's system prompt —
kept producing friction in the wrong places.
The agent forgot to load concepts, expanded scope silently,
and made decisions I should have been making.

The root cause isn't the agent or the prompts.
It's the architecture: an autonomous agent with tools is the wrong shape
for how I want to collaborate.
I want to steer. I want the agent to advise. I want nothing to move without me.

Helm is where that happens.
Where the framework captures how I think about collaboration,
Helm is how I put it into practice at a terminal.

## Voyages

"Resolve issue #42." "Review PR #128." "Investigate flaky CI."
A voyage starts with intent, accumulates a logbook of bearings and actions, and ends with an outcome.

```
$ helm voyage new "Resolve #42: fix widget crash" --kind resolve-issue
a3b0fc12

$ helm voyage list
a3b0fc12  [active] [resolve-issue]  Resolve #42: fix widget crash
```

Voyages are short by design.
The ideal voyage is one session: start it, do the work, complete it.
If you need to stop and come back, record a bearing so the next session has context.

## Bearings

A bearing captures the state of the world at a point in time:
what was observed and what it means.

Observations are the raw data — filesystem state, project structure, whatever the source kind produces.
A position is a short statement about what the observations mean.
Together they form an immutable record in the logbook.

```
$ helm observe rust-project . --out obs.json
$ helm record a3b "Null check missing in widget init path" --observation obs.json
Bearing recorded for voyage a3b0fc12
```

Bearings exist for continuity.
Record one when you'd need context if you stopped and came back in a new session.
If a voyage finishes in one session, `helm voyage complete --summary` is often the only record needed.

## Actions

An action is something that changed the world.
Push a branch, open a PR, merge, comment, close an issue.
Each action records what happened, who did it, and when.

```
$ helm act a3b --as john-agent push --branch fix-widget --message "Fix null check"
$ helm act a3b --as john-agent create-pull-request --branch fix-widget --title "Fix widget"
$ helm act a3b --as john-agent merge-pull-request 45
```

Actions execute real commands (git, GitHub CLI) and record the result.
Failed operations are not recorded — the logbook captures what happened, not what was attempted.

Identity (`--as`) selects which GitHub account to use.
Each identity has its own auth config under `~/.helm/gh-config/`.

## The Logbook

Append-only. Nothing is overwritten. Nothing is dropped.
Bearings and actions interleave in the order they happened.

```
$ helm voyage log a3b
Voyage: Resolve #42: fix widget crash
Status: active

── Bearing 1 ── 2026-02-26T15:05:12Z
  Subject: RustProject @ .
  Position: Null check missing in widget init path

── Action 2 ── 2026-02-26T15:30:00Z
  as: john-agent
  pushed to fix-widget (abc1234)

── Action 3 ── 2026-02-26T15:31:00Z
  as: john-agent
  created PR #45
```

Logbook data lives locally on the machine. Not committed, not synced, not shared.

## Source Kinds

Each source kind is a domain of observable reality — not a mechanism.
Commands are how Helm fetches data; kinds describe what Helm is looking at.

The current set:

- **Files** — filesystem structure and content. Scope: directories to survey. Focus: specific files to inspect.
- **Rust Project** — a Rust project rooted at a directory. Walks the tree, respects `.gitignore`, produces structure and source contents.

Source kinds grow as Helm needs to see more of the world.
GitHub, web resources, and search are natural next additions.

## CLI Shape

Every command is non-interactive: arguments in, structured output out.
Commands compose with shell pipes, files, and scripts.

Helm doesn't own the interactive experience.
[Bridge](https://github.com/dyreby/bridge) provides a TUI that drives Helm
from a single interactive surface.
Helm doesn't need to know Bridge exists —
it serves whoever calls it, whether that's a human, an agent, Bridge, or a shell script.

## Key Concepts

| Name | Role |
|------|------|
| **Voyage** | A unit of work with intent, logbook, and outcome |
| **Logbook** | Append-only voyage history |
| **Bearing** | Immutable record: observations + position |
| **Observation** | Self-contained: subject + sighting + timestamp |
| **Subject** | What you pointed the spyglass at |
| **Sighting** | What you saw |
| **Position** | Short statement of world state |
| **Action** | Immutable record of something that changed the world |
| **Act** | What was done (push, create PR, merge, etc.) |
| **Source Kind** | A domain of observable reality |

## Design Principles

- **Non-interactive.** Every command takes arguments and produces output. No menus, no prompts, no sessions.
- **Composable.** Output is structured (JSON). Commands pipe and chain. Scripts and agents are first-class callers.
- **Append-only.** The logbook is a trail, not a workspace. Nothing is overwritten.
- **Caller-agnostic.** Helm doesn't know who's driving. The approval gate lives upstream.
- **Local.** Voyages and logbooks live on the machine. No syncing, no shared state.
- **Joy matters.** This is a daily tool. The language and the rhythm should feel intentional.

## Inspirations

John Boyd's OODA loop. Michael Singer's practice of serving the moment unfolding in front of us.
My dad's love of sailing.

## What This Document Is Not

- **Not an implementation plan.** The concepts here are the architecture. Implementation emerges through building.
- **Not a general framework.** Helm is shaped by how I think. If any of it is useful to you, that's wonderful.
