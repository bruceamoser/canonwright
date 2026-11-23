# Canonwright Documentation

| Section | Contents |
| --- | --- |
| [Narrative design](creative-writing-architecture.md) | The original architecture plan for the creative-writing harness |
| [Architecture](architecture/README.md) | System design, data model, harness, memory, security |
| [Plans](plans/README.md) | Implementation phases and cache benchmarks |
| [ADRs](adr/README.md) | Accepted architecture decisions |
| [Local Web Services](web-services.md) | SearXNG and Jina Reader provisioning, overrides, and troubleshooting |
| [Brand](brand/logo-brief.md) | Logo brief, mark concepts, and the icon pipeline |
| [First run](first-run.md) | Running the desktop app and exercising the story tools |

## Start here

1. [12 — The Story State Graph](architecture/12-story-state-graph.md) — what
   Canonwright adds to the harness
2. [ADR-010](adr/010-copy-openbad-core-as-a-starting-point.md) — why this
   repository is a copy of OpenBaD rather than a fork
3. [Narrative design](creative-writing-architecture.md) — the original plan
4. [System Overview](architecture/01-system-overview.md) — the inherited runtime
5. [Cache-First Harness](architecture/04-cache-first-harness.md)

## The two invariants

Harness, inherited:

> **The model-visible prefix is immutable and append-only. Volatile context
> goes at the tail, never in the prefix.**

Narrative, added:

> **State is folded from scene deltas, so a snapshot of scene N contains no
> representation of scene N+1.**

## Status

The harness core is inherited from OpenBaD v0.3.31 and builds. The narrative
layer exists as `crates/canonwright-story` (implemented and tested) plus four
read-only `story_*` tools in the registry. Reviewer and editor agents, SQLite
projection of story state, and the repurposed desktop UI are not built.

The documents under `architecture/` other than `12-story-state-graph.md` were
written for the coding harness and are being rewritten. They remain accurate as
descriptions of the runtime Canonwright inherited.
