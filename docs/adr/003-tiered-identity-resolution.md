# ADR 003: Identity resolution and credential routing

**Status:** Accepted
**Date:** 2026-02-27
**Issue:** [#116](https://github.com/dyreby/helm/issues/116)

## Context

Every command that touches GitHub or writes to the logbook needs to know who is acting. Currently `--as` is required on `steer` and `log`, and optional (with a runtime check) on `helm observe` for GitHub targets.

This creates friction in two directions:

**Single-identity users repeat themselves forever.** A developer running `helm steer` dozens of times a day types `--as dyreby` on every invocation. The flag conveys no new information — the identity never changes. It's ceremony.

**Multi-agent setups have no clean session hook.** A TUI or script coordinating agents with different roles needs to pass `--as` per invocation. There's no way to say "this process is acting as agent-x" once and have all commands inherit it.

## Decision

A helm **identity** is a string that serves two purposes simultaneously:

1. **Logbook attribution** — who is recorded as having acted
2. **GitHub credential routing** — which `gh` auth context to use

These are not two separate concerns. An identity that can't authenticate can't steer. An identity that can authenticate but isn't recorded means an unattributed logbook. They're two parts of one pipeline: resolve an identity → use that identity to locate credentials → act → record.

### Credential routing

Helm runs `gh` as a subprocess and controls which GitHub account it authenticates as via `GH_CONFIG_DIR`. Each identity has its own config directory:

```
~/.helm/gh-config/<identity>/
```

Resolving an identity to `dyreby` means setting `GH_CONFIG_DIR=~/.helm/gh-config/dyreby/` on every `gh` call. The identity string is the key. This is convention-based and intentionally simple — no per-identity config overrides, no indirection.

### Identity resolution chain

Identity is resolved in order:

1. `--as <identity>` — explicit per-command override
2. `HELM_IDENTITY` env var — process/session level (a TUI or script sets this once per agent)
3. `~/.helm/config.toml` — global default for single-identity users

A single `resolve_identity(explicit: Option<&str>) -> Result<String>` helper encodes the chain. Commands receive a resolved identity string and don't need to know where it came from.

### The env var name: `HELM_IDENTITY`

`HELM_IDENTITY` maps directly to `--as` — same concept, different layer of the resolution chain. Self-documenting wherever it appears in shell configs or CI pipelines.

The alternative considered was `HELM_CAPTAIN` — nautical, evocative. But `captain` doesn't appear in helm's vocabulary: voyage, logbook, bearing, slate, observe, steer, log. Nautical terms earn their place by carrying meaning. This one would just be a fun name for a string. `HELM_IDENTITY` is the right choice.

### Config: narrow return

`~/.helm/config.toml` comes back scoped to a single field:

```toml
identity = "dyreby"
```

The config field is `identity` — parallel to `--as`, parallel to `HELM_IDENTITY`. The old `Config` was removed because `default_identity` felt like implicit magic. This is different: it's the documented bottom of an explicit resolution chain. A user who sets it is making a conscious choice.

### Error message

When resolution fails on `steer` or `log`:

```
identity required: pass --as <identity>, set HELM_IDENTITY, or add `identity = "..."` to ~/.helm/config.toml
```

All three sources named. No guessing required.

### Per-command behavior

**Steer and log:** identity must resolve. `--as` is optional — for override and explicitness, not ceremony. The logbook records the resolved identity. Resolved identity routes credentials for any GitHub actions steer performs.

**Observe:** GitHub targets resolve identity to locate `~/.helm/gh-config/<identity>/`. Local targets (filesystem, Rust project) don't need identity — resolution is skipped entirely.

**Slate commands:** `helm slate list`, `helm slate erase`, `helm slate clear` are purely local. No identity resolution.

## Consequences

- **Identity is a unified concept.** One string, two roles: attribution and credential key. Adding a new identity means adding a `gh-config` directory — nothing else.
- **Single-identity users set identity once.** In config or as a shell export. No `--as` on every command.
- **Multi-agent setups have a clean hook.** `HELM_IDENTITY=agent-x helm steer ...` is the idiom. The process boundary is the identity boundary.
- **The logbook records who acted.** Every entry is attributed. How identity was determined is invisible to the log — only the resolved value matters.
- **`--as` remains available.** Override when you need to act as a different identity for one command.
- **Config is back, but narrow.** One field, clearly the bottom of an explicit chain.
