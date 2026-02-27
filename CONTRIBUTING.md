# Contributing

## Before Pushing

Run the full CI locally. These match the checks in `.github/workflows/ci.yml`:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Fix any issues before pushing. Don't rely on CI to catch what you can catch locally.

## GitHub CLI Usage

When creating PRs or posting issue comments with `gh`, write anything beyond a sentence or two to a temp file and pass it via `--body-file`:

```bash
body=$(mktemp)
cat > "$body" <<'EOF'
Your multi-line body here.

Can include `backticks`, "quotes", and newlines without escaping.
EOF

gh pr create --title "..." --body-file "$body"
gh issue comment 123 --body-file "$body"
```

Inline `--body` strings don't render `\n` as newlines, and the shell interprets backticks inside them. `--body-file` avoids both problems.

## How Work Is Organized

Work in this project follows a nautical metaphor that mirrors the tool itself.
Why? Because I'm a nerd, it's fun, and I do my best work when I'm enjoying it.

### Commissions

A **commission** is a GitHub issue that organizes a body of work.
It has three parts:

- **Why** — Why does this work matter now?
- **What** — What are we trying to accomplish?
- **Missions** — A prioritized checklist of issues that get us there. Each one requires a voyage to complete.

Each mission is itself an issue with its own **Why** and **What**.
This structure repeats at every level: motivation, objective, then the work to get there.

A commission is not a mission.
It lives above missions and voyages — it organizes them, but isn't the target of one.
A commission closes when all its missions are checked off, not through a voyage.
To create one, use the [commission template](https://github.com/dyreby/helm/issues/new?template=commission.md).

### Voyages

A **voyage** is tracked through the helm CLI.
To start work on a mission, create a voyage for it.
The voyage records the journey — observations, steering actions, and logged states — as an append-only logbook.

### Working a Commission

1. Check the commission — see what's next.
2. Start a voyage for the next mission.
3. Work through it. Observe, steer, log.
4. End the voyage. Check off the mission.
5. Return to the commission.

### Architecture Decision Records

Design decisions that change how helm works are recorded as ADRs in [`docs/adr/`](docs/adr/).
Each ADR captures the context, the decision, and the consequences — why something was decided, not just what changed.

Format: `docs/adr/NNN-short-name.md`.

### Getting Started

Run `helm --help`. That should be all you need to start working a mission.
If it's not, toss a [message in a bottle](https://github.com/dyreby/helm/issues/new?template=message-in-a-bottle.md). No guarantees it washes ashore, but I'll try.
