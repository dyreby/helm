# Vision

Helm is how I want to work with a coding agent.

## Why

An autonomous agent with tools is the wrong shape for how I want to collaborate with an LLM. I want to set the course, trust the agent to crew the voyage, and know what happened — not approve every step.

## Voyages

A voyage starts with intent — where you're going and why. Along the way you take bearings to understand where you are and take actions to change the world. The logbook records what you choose to capture, in order, immutably.

Voyages are short by design. The ideal voyage is one session. If you need to stop and come back, a bearing captures where things stand.

## Bearings

A bearing records what you looked at and what you concluded. The marks tell what it was based on; the reading is your interpretation. Full observations are stored separately — useful for deeper context, but not required to follow the voyage's story.

Scanning readings across bearings tells the voyage's story without replaying raw sightings.

## Actions

An action is something that changed the world — push a branch, open a PR, merge, comment, close an issue. Each records what happened, who did it, and when. The logbook captures what happened, not what was attempted.

## Source Kinds

Each mark describes a domain of observable reality — not a mechanism. Commands are how Helm fetches data; marks describe what Helm is looking at. Source kinds grow as Helm needs to see more of the world.

## Principles

- **Non-interactive.** Arguments in, output out. Commands compose with pipes, files, and scripts.
- **Append-only.** The logbook is a trail, not a workspace.
- **Caller-agnostic.** Helm serves whoever calls it — human, agent, or script.
- **Local.** No syncing, no shared state.
- **I set the course; the agent crews; Helm keeps the log.** Trust the voyage, review the trail.
- **Bring your own tools.** Helm orchestrates; external tools do the specialized work.
- **Joy matters.** This is a daily tool. The language, the flow, and the rhythm should feel intentional and calm.

## Inspirations

Helm is inspired in part by John Boyd's OODA loop, Michael Singer's model of reality as a series of moments unfolding in front of us, and my dad's love of sailing. It's a personal tool, deeply shaped by how I think, tuned to my preferred way of working, and not trying to be general. If it evolves in a way that's useful to you along the way, that's wonderful.

See [DESIGN.md](DESIGN.md) for types and structure.
