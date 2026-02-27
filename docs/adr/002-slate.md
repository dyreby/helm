# ADR 002: Slate — the named working set and its management commands

**Status:** Accepted
**Date:** 2026-02-27
**Issue:** [#113](https://github.com/dyreby/helm/issues/113)

## Context

ADR 001 introduced the observe/steer/log model with a "working set" — observations accumulating between steer/log commands. Three things about the original working set design became friction as the model matured:

**"Working set" is a computer science term, not a nautical one.** The rest of helm's vocabulary is nautical and load-bearing — it shapes how the tool feels to use. "Working set" was a gap.

**The working set had no CLI presence.** You could add to it (`helm observe`) and it cleared automatically on steer/log, but you couldn't inspect it or remove entries. There was no recourse if you observed something wrong or stale — it would seal into the bearing regardless.

**ADR 001 described steer/log as doing automatic curation** — deduplicating, capping size, spilling large payloads to the hold. In practice this conflated two distinct responsibilities: pruning unwanted observations (curation) and committing the remaining observations to a bearing (sealing). Keeping them together meant you couldn't curate without also committing.

## Decision

### Rename: working set → slate

Ships kept a chalk slate on deck for temporary notes — observations, bearings, soundings during a watch. At the end of the watch, entries were transcribed to the logbook and the slate was wiped clean. That maps directly to how helm's accumulating observations work: write to the slate, seal into a bearing, wipe.

The rename makes the temporary→permanent flow intuitive. "Wipe the slate" is the clear operation. "What's on the slate?" is the natural question before deciding to steer. The storage file is `slate.jsonl`.

### `helm slate` subcommand group

A `helm slate` subcommand group manages the slate directly, mirroring the `helm voyage` pattern:

- `helm slate list` — show what's on the slate for a voyage. JSON output by default: helm is caller-agnostic, and agents need to parse the slate to decide what to erase.
- `helm slate erase <target>` — remove all observations of a target before sealing. Erase is target-based: same syntax as `helm observe`, so you erase what you observed by name.
- `helm slate clear` — wipe the slate entirely without sealing. No logbook entry. For when you want to start fresh.

### Separation of curation and sealing

The split between `helm slate erase` and steer/log clarifies the responsibility each holds:

- **`helm slate erase`** curates — remove observations you don't want sealed.
- **`helm steer` / `helm log`** seal — commit whatever remains on the slate into a bearing, then wipe.

Steer and log no longer need to be smart about what they seal. They seal everything on the slate. Curation is explicit and caller-controlled.

This is cleaner than ADR 001's automatic curation model. The invariant is simpler: steer and log seal and clear. Full stop.

### `ObserveTarget` as the shared extension surface

`ObserveTarget` (the CLI enum describing what helm can look at) is shared between `helm observe` and `helm slate erase`. Both commands accept the same target syntax:

```
helm observe --voyage a3b --as dyreby github-issue 42
helm slate erase --voyage a3b github-issue 42
```

Adding a new observation type means adding one variant to `ObserveTarget`. Both commands extend automatically — erase support is free.

`ObserveTarget` lives in `cli/target.rs`, a shared module within the CLI layer. It is a CLI type (Clap-annotated) and does not belong in the model.

## Consequences

- **The slate is first-class.** Visible, manageable, named. Agents can inspect and prune before sealing.
- **Curation is explicit.** No implicit deduplication or capping on seal. What's on the slate is what gets sealed.
- **The extension surface is singular.** One place to add a new observable type. Observe and erase stay in sync automatically.
- **The nautical vocabulary closes.** Slate, seal, wipe — the metaphor is complete and consistent.
- **ADR 001's automatic curation is superseded** for the pruning aspect. Deduplication on seal (keeping the newest observation per target) is retained as a cheap safety net, not the primary curation mechanism.
