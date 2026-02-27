# Helm

A personal, Rust-based CLI for structured collaborative software work.

Helm observes the world, acts upon it, and records everything in an append-only logbook.
Three commands (**observe**, **steer**, **log**) and one invariant: only steer and log write to the logbook.

```
helm observe issue 42        # look at something, add to the working set
helm steer comment 42 "..."  # act on collaborative state, seal and log
helm log waiting              # record a state, seal and log
```

Work is organized into **voyages** — units of work, each with its own logbook.
The logbook tells the story: what was observed, what decisions were made, what changed in the world.

I'm working on a [collaboration framework](https://github.com/dyreby/collaboration-framework) that started as a way to work better with a coding agent and evolved into something broader about narrowing the gap between intent and understanding.
The concepts work, but the mechanism (shared concepts injected into an agent's system prompt) kept producing friction.
Helm picks up where that approach ran into its limits, replacing prompt engineering with structured workflows.

Inspired in part by John Boyd's OODA loop, Michael Singer's model of reality as a series of moments unfolding in front of us, and my dad's love of sailing.

**This is a personal tool.** Deeply shaped by how I think, tuned to my preferred way of working, and not trying to be general. If it resonates with you, that's great — explore, ask questions, take ideas, fork it. If you adapt something and it works well for you, I'd love to hear about it.

See [CHARTER.md](CHARTER.md) for the purpose, [VISION.md](VISION.md) for the approach, and [DESIGN.md](DESIGN.md) for the specifics.
