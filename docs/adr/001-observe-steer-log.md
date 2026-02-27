# ADR 001: Observe / Steer / Log Command Model

**Status:** Accepted
**Date:** 2026-02-27
**Issue:** [#92](https://github.com/dyreby/helm/issues/92)

## Context

Helm's original design had three phases: observe (gather data), bearing (interpret what was observed), action (do something in the world). After using this model, several friction points emerged:

**Manual bearing-taking interrupts the OODA loop.** Observations and interpretations are separate commands with separate artifacts. In practice, taking a bearing is overhead — you want to observe freely, then commit when you're ready to act. The interpretation step should happen at decision time, not as a standalone ritual.

**Actions are too low-level.** Logging individual git commits and pushes creates noise. A commit is an implementation detail. What matters for collaborative software work is when state transitions become shared — when an issue gets a comment, a PR gets opened, a review gets addressed. Local git mechanics don't belong in the logbook.

**The agent contract scopes helm too narrowly.** The original vision framed helm as "how I want to work with a coding agent," with a stateless agent contract (take bearing, correct reading, correct course) built into the design. After broadening the vision (#90) to "how I want to navigate collaborative software work," the agent contract no longer belongs in helm. What produces decisions — human, agent, script — is outside helm's scope.

**The terminology carries dead weight.** Marks, sightings, readings, reading challenges, action kinds — these concepts served the original three-phase model but add indirection without clarity. The nautical metaphor should enhance understanding, not require a glossary.

## Decision

Replace the observe/bearing/action model with three commands: **observe**, **steer**, and **log**.

### Commands

- **`helm observe`** gathers observations into a per-voyage working set. It never writes to the logbook. Observations are cheap, frequent, and ephemeral.
- **`helm steer`** executes an intent-based action that mutates collaborative state. It seals a bearing from the working set, executes the action, writes one logbook entry, and clears the working set. All atomically.
- **`helm log`** records a deliberate state (waiting, blocked, ready) without mutating collaborative state. Same seal-and-clear behavior as steer.

Only steer and log write to the logbook. That's the invariant.

### Working set and automatic bearing curation

Observations accumulate between steer/log commands. When either is called, helm curates the working set into a bearing automatically — deduplicating, capping size, spilling large payloads to the hold. No manual bearing-taking step.

### Collaborative state as the boundary

Steer actions represent meaningful state transitions that cross the collaborative boundary — not API calls, not git operations. GitHub is the first implementation of that boundary, but the model doesn't assume it.

### Simplified terminology

| Term | What it means |
|------|---------------|
| **Voyage** | A unit of work with a logbook |
| **Observation** | What you looked at + what came back + timestamp |
| **Bearing** | Curated observations + summary, sealed into a log entry |
| **Working set** | Observations accumulating between steer/log commands |
| **The hold** | Per-voyage content-addressed storage for large payloads |
| **Steer** | Intent-based action that mutates collaborative state |
| **Log** | Record state without mutating collaborative state |

Mark, sighting, reading, and action kind are gone as separate concepts. Observations use plain field names: `target` (what you looked at), `payload` (what came back).

### What's removed

- The `helm bearing` command and manual bearing-taking workflow.
- The reading/challenge mechanism.
- The agent contract (take bearing, correct reading, correct course).
- Local git action types (Commit, Push).
- The `helm action` command.

## Consequences

- **The OODA loop gets faster.** Observe freely, steer or log when ready. No intermediate steps.
- **The logbook tells a cleaner story.** Entries are meaningful state transitions, not implementation details.
- **Helm becomes caller-agnostic in practice, not just principle.** No agent contract means no assumptions about what drives decisions.
- **Steer subcommands become the extension surface.** Adding a capability means adding a steer subcommand — a deterministic flow with a known shape.
- **Existing code changes significantly.** The CLI, storage layout, and model types need rework. The observation modules (filesystem, GitHub) largely survive.
- **DESIGN.md is rewritten** to reflect the new model.
