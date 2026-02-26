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

A voyage starts with intent, accumulates a trail of bearings and actions, and ends with an outcome.
Voyages are short by design — the ideal voyage is one session.
If you need to stop and come back, record a bearing so the next session has context.

## Bearings

A bearing captures the state of the world at a point in time.
Observations are the raw data — what a source kind produces when pointed at the world.
A position is a short statement about what the observations mean.
Together they form an immutable record in the logbook.

Bearings exist for continuity, not documentation.
If a voyage finishes in one session, the completion summary is often the only record needed.

## Actions

An action is something that changed the world — push a branch, open a PR, merge, comment, close an issue.
Each records what happened, who did it, and when.
Failed operations are not recorded; the logbook captures what happened, not what was attempted.

## The Logbook

Append-only. Nothing is overwritten. Nothing is dropped.
Bearings and actions interleave in the order they happened.
Logbook data lives locally on the machine. Not committed, not synced, not shared.

## Source Kinds

Each source kind is a domain of observable reality — not a mechanism.
Commands are how Helm fetches data; kinds describe what Helm is looking at.
Source kinds grow as Helm needs to see more of the world.

## CLI Shape

Every command is non-interactive: arguments in, structured output out.
Commands compose with shell pipes, files, and scripts.

Helm doesn't own the interactive experience.
[Bridge](https://github.com/dyreby/bridge) provides a TUI that drives Helm from a single interactive surface.
Helm doesn't need to know Bridge exists —
it serves whoever calls it, whether that's a human, an agent, Bridge, or a shell script.

## Principles

- **Non-interactive.** Arguments in, output out. No menus, no prompts, no sessions.
- **Composable.** Structured output. Commands pipe and chain. Scripts and agents are first-class callers.
- **Append-only.** The logbook is a trail, not a workspace.
- **Caller-agnostic.** Helm doesn't know who's driving. The approval gate lives upstream.
- **Local.** No syncing, no shared state.
- **Joy matters.** The language and the rhythm should feel intentional.

## Inspirations

John Boyd's OODA loop. Michael Singer's practice of serving the moment unfolding in front of us.
My dad's love of sailing.
