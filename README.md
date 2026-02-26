# Helm

A personal, Rust-based CLI for disciplined collaboration with a coding agent.

You set the course, the agent crews. The helm awaits. ⎈

Inspired in part by John Boyd's OODA loop, Michael Singer's model of reality as a series of moments unfolding in front of us, and my dad's love of sailing.

I'm working on a [collaboration framework](https://github.com/dyreby/collaboration-framework) that started as a way to work better with a coding agent and evolved into something broader about narrowing the gap between intent and understanding. The concepts work, but the mechanism — prompt extensions injected into an agent's system prompt — kept producing friction. The agent forgot context, expanded scope silently, and made decisions I expected to make. The root cause is architectural: an autonomous agent with tools is the wrong shape. Helm picks up where that approach ran into friction — replacing prompt engineering with structured, artifact-driven workflows.

**This is a personal tool.** It reflects how I think and work. If it resonates with you, that's great — explore, ask questions, take ideas, fork it. If you adapt something and it works well for you, I'd love to hear about it.

See [CHARTER.md](CHARTER.md) for the purpose, [VISION.md](VISION.md) for the approach, and [DESIGN.md](DESIGN.md) for the specifics.
