# 12 — The Story State Graph

This document describes `crates/canonwright-story`, the part of Canonwright that
is not inherited from the harness. Source of the design intent:
`docs/creative-writing-architecture.md` (the original plan), and
[ADR-010](adr/010-copy-openbad-core-as-a-starting-point.md) (why it lives in a
copied tree).

## Why a graph and not a wiki

A wiki is a set of statements that are all true *now*. A story is a set of
statements that become true at particular points, are believed by some people
and not others, and stop being true when a character dies or a faction falls.

The difference is not organisational. It decides whether the harness can answer
the questions a reviewer actually asks:

| Question | Flat Markdown | Story State Graph |
| --- | --- | --- |
| What does Mara know in chapter 9? | unanswerable | `knowledge_at(scene, line, mara)` |
| Is this reveal already spoiled? | unanswerable | `state_before(scene).known_to_reader` |
| Which chapters break if I change this scene? | unanswerable | `dirty_after(scene)` |
| Is this reviewer's complaint real? | opinion | `substantiate(ticket)` |
| What did the blockade cost, three chapters on? | unanswerable | folded `Resource` deltas |

## Shape

```rust
Story {
    meta,
    characters: { CharacterId → Character },   // base truth, before scene 1
    factions:   { FactionId   → Faction },
    rules:      { RuleId      → Rule },        // Hard | Soft
    facts:      { FactId      → Fact },        // secret | revealed
    threads:    { ThreadId    → Thread },      // planted_in, promised_payoff
    scenes:     [ Scene ],                     // ordered by index, not position
}

Scene {
    id, index, pov, present, location,
    status: Planned | Drafted | Reviewed | Canon,
    value_start, value_end,                    // the value shift
    dependencies: [ CausalLink { Because | But | Therefore, scene } ],
    delta: SceneDelta { changes: [ SceneChange { at_line, Change } ] },
}
```

Base entities describe what is true *before the story starts*. Everything that
happens afterwards is a `Change` inside a scene's delta. There is no third place
for state to live.

`Change` covers the moves a scene can make:

```rust
Learned { character, fact }          Revealed { fact }
Relationship { from, to, axis, value }
Resource { faction, resource, delta } Posture { faction, posture }
Injury { character, .. }              Death { character }
Arrival { character, location }
ThreadPaidOff { thread }              ThreadAbandoned { thread }
CanonNote { note }
```

## Folding

`state_before(scene)` seeds from the base entities and replays the deltas of
every scene with a lower index whose status contributes state (drafted or
later). Two consequences fall out for free:

- A `Planned` scene contributes nothing. A beat on the board cannot kill anyone.
- A snapshot is bounded by construction. There is no future in the structure, so
  there is nothing for a prompt to accidentally include.

Relationships are absolute, not additive: an author says "trust is now 2", not
"trust +1". Axes clamp to `[-5, 5]`, so one scene cannot rewrite a bond.

Knowledge is line-aware. A change with `at_line = 12` takes effect at line 12;
`knowledge_at(scene, 9, mara)` does not include it, and `knowledge_at(scene, 20,
mara)` does. A change with no line counts only after the scene ends, which is
the strict reading — an accusation of a knowledge leak should have to clear a
bar.

## Dirty state

Causality is declared as `because` / `but` / `therefore` edges pointing at
earlier scenes. Reversing the edges gives the propagation graph:
`dirty_after(s)` is the transitive closure of scenes downstream of `s`,
restricted to scenes that already exist as prose. Editing scene 2 tells you that
scenes 4 and 7 need a continuity pass and scene 5 does not.

## The two kinds of finding

**`check()` is the compiler.** Unknown ids, duplicate scene indexes, a
dependency on a later scene, a thread whose promised payoff precedes its plant.
These are returned as `StoryError` because no reading of the story makes them
correct.

**`lint()` is the linter.** No value shift, no causal link, a character present
after dying, a plant cold for six scenes, a payoff promised and not delivered.
These are `Ticket`s, and every one is advisory.

A ticket may carry a machine-checkable `Claim`:

```rust
Claim::Knowledge { character, fact }   // resolved against knowledge_at
Claim::Thread { thread }               // resolved against thread status
Claim::Rule { rule }                   // hard rules bind; soft lore does not
Claim::ValueShift                      // resolved against value_start/end
```

`substantiate(ticket)` returns `Confirmed`, `Refuted(reason)`, or `Uncheckable`.
This is the evidence gate, and it is what stops a reviewer from inventing
continuity errors: a claim about the timeline is settled by the timeline.

## Prompt assembly

`context_for(scene)` renders a Markdown brief: the scene's value at stake, how we
got here (with causal kinds), the *present* cast with their wants, voice
constraints, injuries and knowledge, the threads still owed, and what the reader
knows. It reports what it withheld — later scenes, unrevealed facts, facts the
POV has not learned, unplanted threads — so the author can see the filter working
instead of trusting that it did.

## Storage

A story is a JSON document on disk at `.canonwright/story.json` today, written
atomically. SQLite projection is planned; the crate deliberately does not depend
on `canonwright-store`, so the projection can be added without touching the
model.

## What is not built yet

- Reviewer and editor agents, and the concurrency that runs them.
- A `story_apply` tool and canon commit with approval.
- Persistence in the store and querying briefs as SQL.
- Multi-scene arcs: the model has scenes, not acts or books.
