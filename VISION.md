# Vision

Helm structures work as voyages.
Each voyage has an intent, an append-only logbook, and an outcome.

## Voyages

A voyage starts with intent — where you're going and why.
Along the way you take bearings to understand where you are
and take actions to change the world.
The logbook records everything that happened, in order, immutably.

Voyages are short by design.
The ideal voyage is one session.
If you need to stop and come back, a bearing captures where things stand.

## Bearings

A bearing captures the state of the world at a point in time.
Observations are the raw data — what you see when you point the spyglass at a source kind.
A position is a short statement about what the observations mean.
Together they form an immutable record in the logbook.

Bearings exist for continuity, not documentation.
If a voyage finishes in one session, the completion summary is often the only record needed.

## Actions

An action is something that changed the world —
push a branch, open a PR, merge, comment, close an issue.
Each records what happened, who did it, and when.
The logbook captures what happened, not what was attempted.

## Source Kinds

Each source kind is a domain of observable reality — not a mechanism.
Commands are how Helm fetches data; kinds describe what Helm is looking at.
Source kinds grow as Helm needs to see more of the world.

## Principles

- **Non-interactive.** Arguments in, output out. Commands compose with pipes, files, and scripts.
- **Caller-agnostic.** Helm serves whoever calls it — human, agent, TUI, or script.
- **Append-only.** The logbook is a trail, not a workspace.
- **Local.** No syncing, no shared state.
- **Joy matters.** Naming and language shape how a tool feels to use. I find the nautical theme helpful and fun here.

## Inspirations

Helm is inspired by, among other things, John Boyd's OODA loop, Michael Singer's model that reality is a series of moments unfolding in front of us, and my dad's love of sailing. It's a personal tool, deeply shaped by how I think, tuned to my preferred way of working, and not trying to be general.
