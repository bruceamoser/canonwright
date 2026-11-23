# Creative Writing Agent Harness: Architectural Plan

## Overview & Core Philosophy

In a software engineering harness, correctness is enforced by a compiler, a linter, and unit tests. In long-form creative writing, there is no compiler—only **narrative causality, continuity, character agency, and thematic resonance**. 

Treating story elements as flat Markdown files in a standard RAG pipeline inevitably causes LLMs to flatten character voices, leak dramatic irony, or break world rules chapters later. To solve this, **plot, characters, world, factions, and motivations must be treated as first-class citizens**—modeled as a strongly-typed, temporal state graph (analogous to an Abstract Syntax Tree) rather than passive background text.

---

## 1. The Story State (First-Class Narrative Primitives)

Before defining the agents, the harness must define the structured, versioned **Story State Graph** that agents read from and propose mutations to. Rather than a single static wiki, this state should be **event-sourced or snapshot-based by scene/chapter** so agents only see what is true *at that exact point in the timeline*.

### Core Entities & Schemas

* **Characters**
  * **Motivations:** Macro arc (*Need*), current active goal (*Want*), and immediate scene objective.
  * **Knowledge Boundaries:** Explicit tracking of what the character knows vs. what is hidden from them (preventing accidental knowledge leaks).
  * **Voice & Lexicon Profile:** Sentence rhythm, vocabulary constraints, verbal tics, and emotional tells.
  * **Relationship Vectors:** Directed edges to other characters and factions (e.g., `Trust`, `Fear`, `Debt`, `Resentment`) with brief justifications.
* **Factions**
  * **Resources & Territory:** Tangible assets, economic leverage, and physical control.
  * **Current Posture:** Strategic stance (e.g., *expansionist, fractured, covert, defensive*).
  * **Internal Friction:** Sub-factions, rival leaders, and ideological fault lines (ensuring factions never act as simplistic monoliths).
* **World & Lore**
  * **Hard Rules:** Non-negotiable constraints (magic/technology costs, physics, geography, travel times, seasonal weather).
  * **Soft Lore:** Cultural myths, unreliable histories, idioms, and local customs.
* **Plot & Causality Graph**
  * **Causal Edges:** Scenes are linked not just sequentially, but via **Because / But / Therefore** causal dependencies.
  * **Dirty-State Tracking:** Like a build system, if Scene 2 is edited in a way that alters its output state, downstream dependent scenes (e.g., Scene 5) are flagged as "dirty" and queued for continuity review.
  * **Threads & Payoffs:** Active mysteries, promises made to the reader, Chekhov’s guns, and their resolution status.

---

## 2. Specialized Agent Roster (By Role & Domain)

Splitting agents along two orthogonal axes—**Function** (Collaborator, Reviewer, Editor, State Extractor) and **Domain** (Plot, Character, World, Prose)—prevents context dilution and stops an agent from rewriting prose when it should be auditing causality.

### A. The Collaborators (Generative Writers' Room)
*Operate upstream of prose drafting to brainstorm, stress-test ideas, and expand the Story State Graph.*

* **Character Collaborator:** Simulates how a character reacts to a proposed beat based on their current motivations, trauma, and relationship vectors. Flags when a planned plot point feels forced for that character and pitches organic alternatives.
* **Faction & World Collaborator:** Simulates systemic, second- and third-order consequences. When an event occurs (e.g., a trade blockade or assassination), it calculates ripple effects across neighboring factions, supply chains, and political alliances.
* **Plot Architect:** Focuses strictly on macro and micro structure—tension curves, pacing, turning points, and setup/payoff ratios. Bridges narrative gaps without relying on contrivances.

### B. The Reviewers (Read-Only Diagnostic "Linters")
*Never rewrite prose. They inspect a draft against the active Story State snapshot and emit structured critique tickets (analogous to compiler warnings or PR review comments).*

* **Character Reviewer (Continuity & Agency):**
  * Audits for **Out-of-Character (OOC) drift** against established traits and voice profiles.
  * Detects **Knowledge Leaks** (e.g., a character acting on information they haven't learned yet in the timeline).
  * Flags **Passive Protagonist** syndrome where events happen *to* the POV character rather than being driven by their choices.
* **Plot & Causality Reviewer:**
  * Verifies that the scene changes the state of the story (value shift from start to end).
  * Checks causal links to prior scenes and flags dangling setups or unearned resolutions.
* **World & Lore Reviewer:**
  * Enforces hard rules (travel distances, timeline math, magic/tech limitations, injuries carrying over between scenes).
* **Subtext & Dialogue Reviewer:**
  * Inspects dialogue and blocking to flag "on-the-nose" exposition where characters state their emotions or plot mechanics directly instead of speaking through subtext and conflicting objectives.

### C. The Editors (Targeted Prose Mutators)
*Operate downstream once the author accepts specific Reviewer tickets, performing surgical passes on the text.*

* **Line & Voice Editor:** Refines cadence, sensory grounding, word choice, and dialogue rhythm to match each speaker's voice profile without altering plot mechanics.
* **Pacing & Tension Editor:** Tightens bloated exposition, expands rushed emotional beats, and sharpens scene openings and hooks.

### D. System Utility Agents
* **Context Assembler:** Queries the Story Graph prior to any agent run to build a lean, scene-specific context payload (active POV knowledge boundary, present characters, immediate causal lead-in).
* **State Extractor (Diff Generator):** Reads a finalized scene and proposes structured updates to the Story Graph (new knowledge gained, relationship shifts, resource changes, injury states) for human approval.

---

## 3. Execution Lifecycle: The Scene Commit Pipeline

Mirroring a `Plan -> Code -> Test -> Lint -> Commit` loop, each scene moves through a structured pipeline:

1. **Scene Brief Assembly:** The harness pulls the temporal snapshot of the Story State for Scene $N$, filtering out future arc spoilers and irrelevant background lore.
2. **Beat Ideation (Optional):** The author brainstorms scene beats with the **Plot Architect** and **Character Collaborator**.
3. **Drafting:** The author writes the scene (or generates a rough beat-by-beat draft).
4. **Parallel Lint Pass:** The **Character**, **Plot**, **World**, and **Subtext Reviewers** run concurrently against the draft, returning categorized, line-referenced tickets (`[KNOWLEDGE_LEAK]`, `[VOICE_DRIFT]`, `[CAUSALITY_BREAK]`, `[ON_THE_NOSE]`).
5. **Triage & Revision:** The author dismisses false positives (e.g., intentional character breaks), manually revises, or dispatches accepted tickets to the **Line/Voice** or **Pacing Editors**.
6. **Canon Commit (State Mutation):** The **State Extractor** analyzes the approved scene and outputs a state diff. Once approved by the author, the Story State Graph updates, locking in the new canon for Scene $N+1$.

---

## 4. Key Architectural Guardrails

* **Preventing Over-Constrained Stifling:** Great fiction relies on characters breaking under pressure or unexpected twists. Reviewer tickets should act as **advisory warnings ("friction alerts")**, never hard build-blockers.
* **Temporal Isolation (Anti-Spoiler Context):** Never feed a character’s end-of-book arc summary into early-chapter drafting or review prompts. Agents should only receive state snapshots up to the current scene index to prevent premature foreshadowing or personality bleed.
* **Separation of Critique and Generation:** Keep Reviewer prompts strictly analytical (JSON/structured ticket outputs) and Editor prompts strictly generative. Combining them in a single pass degrades diagnostic rigor.