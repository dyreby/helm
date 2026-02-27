# ADR 003: Helm always fetches full data when observing

**Status:** Accepted
**Date:** 2026-02-27
**Issue:** [#119](https://github.com/dyreby/helm/issues/119)

## Context

`helm observe github-pr` was introduced with a `--focus` flag accepting `summary` or `full`. The intent was to let callers trade off data volume against cost — fetch less when you only need a quick look.

In practice, this creates more problems than it solves.

**An incomplete bearing leads to poor steering decisions.** Helm is non-interactive and agent-first. When an agent observes a PR, it needs the full picture — metadata, comments, diff, files, checks, inline reviews — to make a good decision. Fetching a summary doesn't reduce the agent's need; it reduces what's available when the decision gets made. The agent either works with incomplete information or has to observe again.

**`--focus` conflates two distinct concerns: data collection and context management.** Helm's job is to gather observations faithfully. What the caller includes in its context window — what it passes to a model, summarizes, or discards — is a separate concern that belongs to the caller. Encoding that tradeoff inside helm couples the tool to the caller's resource management strategy.

**`--focus` blocked structural equality in the observe model.** `Observe::GitHubPullRequest` carried a `focus` field that wasn't part of the resource's identity — the same PR with different focus values was the same resource. This required special-case matching in `helm slate erase`, where identity determines what gets erased. Removing `focus` makes structural equality work naturally across the model.

## Decision

Remove `--focus` / `PrFocusArg` from `helm observe github-pr`. Always fetch full data: metadata, comments, diff, files, checks, and inline reviews.

Helm's responsibility is to give the caller a complete picture of what it observed. The caller's responsibility is to decide what enters its context.

This is the invariant: **helm observes fully; the caller filters.**

`Observe::GitHubPullRequest` becomes `{ number: u64 }` — identity only, no strategy embedded in the type.

## Consequences

- **Observations are always complete.** An agent sealing a bearing from a PR observation has the full picture. No second observe needed because something was missing.
- **The caller controls its own context.** What gets passed to a model, summarized, or discarded is the caller's choice — helm doesn't constrain it upstream.
- **Structural equality holds.** `Observe::GitHubPullRequest { number }` has one representation per resource. `helm slate erase github-pr 42` matches correctly without special cases.
- **The CLI surface shrinks.** One fewer flag, one fewer concept to explain or misuse.
- **Data volume increases.** Full fetches cost more than summary fetches. This is the right tradeoff: prefer correctness and completeness over premature optimization at the observation layer.
