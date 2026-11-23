# First run

What you can actually do today, in the order that gets you to a story fastest.

## Prerequisites

- Rust 1.85+ and Cargo
- Node 20+ and pnpm
- Linux only: `libwebkit2gtk-4.1-dev libgtk-3-dev libappindicator3-dev librsvg2-dev libdbus-1-dev pkg-config patchelf`
- A DeepSeek API key, either in the environment as `DEEPSEEK_API_KEY` or entered
  in **Settings → Providers**. The app opens without one; the agent does not run
  without one.

## Run it

```sh
cd apps/desktop
pnpm --dir ui install          # once
./ui/node_modules/.bin/tauri dev
```

That single command installs nothing else, starts Vite for the UI, builds the
Rust side, and opens the window. The first Rust build takes a few minutes; after
that it is incremental.

You should see a window titled **Canonwright**, the logo in the menu bar, and
"narrative harness" under the project name.

### If the window is blank or says "connection refused"

The window needs the Vite dev server. `tauri dev` starts it, but if you launched
`target/debug/canonwright-desktop` directly there is nothing to serve the UI.
Either use `tauri dev`, or build a self-contained binary:

```sh
cd apps/desktop
pnpm --dir ui build
./ui/node_modules/.bin/tauri build --no-bundle   # binary only, embeds the UI
```

### Other desktops

Tauri needs a display. On a headless box:

```sh
export WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1 LIBGL_ALWAYS_SOFTWARE=1
xvfb-run -s "-screen 0 1400x900x24" ./ui/node_modules/.bin/tauri dev
```

## Try the story layer

The narrative tools are reachable through the agent, so this is a conversation.

1. Make a scratch folder and drop a draft in it:

   ```sh
   mkdir -p ~/stories/salt-road && cd ~/stories/salt-road
   printf '# Chapter One\n\nThe road was closed.\n\n***\n\nShe waited.\n' > draft.md
   ```

2. In the app: **New project** → choose `~/stories/salt-road`.

3. Ask the agent:

   > Run `story_intake` with mode existing on `draft.md`.

   It reads the draft first and answers what a document can: the title, the point
   of view (from pronouns), the tense (from verbs), the word count, the scene
   count. Everything else — genre, logline, who the protagonist is, what they
   want, the rule of the world — is still asked, and every question but the title
   can be skipped.

4. Start the story with those answers:

   > Start a story from `draft.md` with `story_start` mode existing, using the
   > intake answers.

   It should report 2 scenes, format markdown, and a placeholder narrator.

5. Then:

   > Read the story with `story_read`, then run `story_lint`.

   Lint will report `NO_VALUE_SHIFT` for each scene. That is correct, not a bug:
   the import knows the prose and not the state.

6. And:

   > Show me the story brief for scene 2.

   The brief lists the cast, the causal lead-in, and what the point of view
   knows at the top of the scene — with the later scenes withheld.

7. Inspect the result on disk: `.canonwright/story.json`. It is plain JSON and
   hand-editable; a worked example with real state lives at
   `crates/canonwright-story/tests/fixtures/the-salt-road.json`.

## What is real, and what is not

Working: import (.md/.txt/.docx/.epub/.pdf), segmentation, the story graph,
temporal state, knowledge-leak checking, the evidence gate, canon commits, the
dirty-scene queue, and eight `story_*` tools wired into the agent.

Not working yet:

- **State extraction.** An imported scene has no cast knowledge, no facts
  learned, no threads planted. The agent can infer them from the prose, but
  nothing writes them back automatically. `story_brief` stays thin until you
  fill the graph in.
- **The new/existing prompt.** `story_intake` and `story_start` implement both
  branches and the question script is deterministic, but the UI does not render
  it yet — you ask the agent.
- **The rest of the UI.** It is still the coding harness with new copy: the
  sidebar is repositories, sessions, and retrieval. None of it is a manuscript
  view.

## Report back

The useful bug reports at this stage are: anything that panics or hangs, a
manuscript that imports to the wrong number of scenes, and a scene boundary that
is clearly wrong. Quote the file name and the expected scene count.
