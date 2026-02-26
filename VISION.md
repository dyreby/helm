# Vision

Helm is how I want to work with a coding agent.

## Why

I'm working on a [collaboration framework](https://github.com/dyreby/collaboration-framework) that started as a way to work better with a coding agent and evolved into something broader about narrowing the gap between intent and understanding. The concepts work. The philosophy holds. But the mechanism I was using (prompt extensions injected into an agent's system prompt) kept producing friction in the wrong places. The agent forgot to load concepts, worked around tool constraints, expanded scope silently, and made decisions I should have been making.

The root cause isn't the agent or the prompts. It's the architecture: an autonomous agent with tools is the wrong shape for how I want to collaborate with an LLM. I want to steer. I want the agent to advise. I want nothing to move without me.

Helm is a companion to the collaboration framework. Where the framework captures how I think about collaboration generally, Helm is how I put that into practice at a terminal with a coding agent.

## What

Helm is a CLI written in Rust and inspired by, among other things, John Boyd's OODA loop, Michael Singer's practice of serving the moment unfolding in front of us, and my dad's love of sailing.

It delegates to external tools for specialized work — an LLM for generating readings and collaborating on course corrections, an editor for reviewing text before it leaves the system. Which tools fill those roles is a preference, not a dependency.

## Voyages

A voyage is a unit of work with a destination. "Review PR #128." "Fix bug: Safari login." "Implement feature: RBAC." Each voyage has an intent, a logbook, and an outcome.

Voyages are short by design. The ideal voyage is one command, a few steps, and a clean exit. Over time, most should converge toward: take bearing, decide on action, confirm, done.

## The Core Loop

### Take Bearing

Observe the world and orient. Everything that follows depends on what's here.

Point the spyglass at a mark — a domain of observable reality. Survey for the broad view, inspect for depth. Take as many observations as you want; keep the ones that matter, discard the rest. Seal the bearing with a reading: your interpretation of what you saw.

### Correct Reading

Refine the agent's interpretation. Sometimes the reading is wrong, or it misses something I know. Challenge it, add context, guide the agent toward accuracy. The goal is a reading I trust before deciding what to do next.

### Correct Course

Decide what to do and shape how to do it. This is where collaboration happens — talking with the agent about approach, reviewing drafts, iterating, editing. Conversation and editing interleave in whatever order makes sense.

This phase ends with either new marks to observe or an action plan.

### Take Action

Affect the world. Post a review, open a PR, apply a patch, create files. This is the only phase where changes can't be taken back.

## The Logbook

A voyage's logbook is append-only. Nothing is overwritten. Nothing is dropped.

Voyages can be paused and resumed. When you return, the world may have changed. Take a new bearing. Old bearings remain as history; the latest bearing is the active ground truth.

## Design Principles

- **Marks describe what to observe, not how to observe it.** Kinds are domains of reality. Commands are implementation.
- **Observations are always fresh.** Survey re-runs every time. No stale catalog reuse.
- **The agent proposes; Helm enforces; I approve.** Authority without autonomy.
- **Immutable history, append-only log.** Nothing is overwritten or dropped.
- **Old sightings are never reused by the agent.** If it needs past data, it proposes a new observation.
- **Voyages are local.** No syncing, no shared state, no committed artifacts.
- **Bring your own tools.** Helm orchestrates; external tools do the specialized work.
- **Joy matters.** This is a daily tool. The language, the flow, and the rhythm should feel intentional and calm.

## What This Document Is Not

- **Not an implementation plan.** See [DESIGN.md](DESIGN.md) for types and structure.
- **Not a general framework.** Helm is shaped by how I think, and it will evolve as I do. If any of it is useful to you along the way, that's wonderful.
- **Not an autonomous agent.** The agent never drives. It advises within hard constraints. The helm waits for me.
