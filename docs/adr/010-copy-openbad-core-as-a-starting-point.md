# ADR-010 — Copy OpenBaD core as a starting point

- **Status:** Accepted
- **Date:** 2026-05-23

## Context

Canonwright is a second product built on the OpenBaD harness. The two share a
runtime shape — a turn/step agent loop, a tool registry with typed schemas, an
event-sourced session log, a cache-first prompt assembler, approvals and risk
levels, MCP, a PTY, and a Tauri shell — and they will not share a product.

OpenBaD must stay a general-purpose harness. That requirement is not
sentimental: it is the thing that makes it useful for coding, research, and
whatever comes next, and it is the thing that breaks the moment fiction-specific
decisions start leaking into its tool schemas and prompt sections.

Two mechanics were available: fork, or copy.

## Decision

**Copy the OpenBaD tree at `v0.3.31` (`3b59840`) with `git archive` into a fresh
repository, and treat the two projects as independent from that moment.**

- No shared git history, so no upstream remote and no merge conflicts to
  negotiate.
- The MIT licence requires that the copyright notice travel with the code; it
  does, in `LICENSE` and `NOTICE`.
- The import is a single commit of the pristine tree, so every change since is
  visible as our own work rather than as a rebase artifact.
- `archive/openbad-v0` — the retired Python/LangGraph prototype — was dropped.
  It is not part of the harness and is preserved upstream.

Crate names, environment variables, the application identifier, the workspace
metadata directory (`.openbad` → `.canonwright`), and binary names were renamed
in the commit that follows the import, so the difference between "what we
inherited" and "what we changed" is a reviewable diff.

## Consequences

- **No upstream merges.** A fix that lands in OpenBaD is not automatically here.
  Divergence is the price of independence, and it is the intended price: the
  harness is roughly 69k lines, and the parts we actually want to track
  (`openbad-core`, `-harness`, `-llm`) are small and stable.
- **Vocabulary carries over.** Terms like "workspace", "session", "tool", and
  "approval" were designed for code and mostly fit prose with a different noun.
  The ones that do not fit — `project areas`, `repo map`, `symbol search` — will
  be removed rather than renamed, because a fiction harness that ships a
  repository map is telling the author what it thinks of them.
- **The UI is inherited wholesale.** It says "repository" where it should say
  "manuscript" and offers git actions that a novelist does not need. Repurposing
  it is planned work, not done work.
- **Attribution is a standing obligation.** If this repository ever ships
  binaries, the notice ships with them.

## Alternatives considered

- **Fork on GitHub.** Rejected: a fork invites comparison, implies a tracking
  relationship, and makes it look like Canonwright is a branch of OpenBaD rather
  than a product.
- **Shared crates via git or path dependencies.** Rejected for now: it couples
  release cadence, and the seams we would share (`core`, `harness`, `llm`) are
  the ones most likely to need narrative-specific changes. Revisit if the two
  projects settle into genuinely common infrastructure.
- **Write a fresh harness.** Rejected: the interesting problem is narrative
  state, not process supervision, streaming, or secret handling.
