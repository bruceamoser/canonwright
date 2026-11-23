# Canonwright — UX reference

The product's screens, drawn before any of the shell was rebuilt. This is the
**reference**, not a description of what the app does today: `apps/desktop` is
still the inherited harness, and these screens are what it is being moved
towards.

Open [`index.html`](index.html) to browse them with captions, or read on.

![The six screens at a glance](contact-sheet.png)

## The set

The order matters: the first two screens are about the machine, the next two
about the book, and the rest about the work.

| Screen | What it settles |
| --- | --- |
| [`00-first-run---github.png`](screens/00-first-run---github.png) | First run. Git and GitHub before anything else, because an existing manuscript is often already a repository. Device code, "skip for now", and what has already been checked. |
| [`01-welcome.png`](screens/01-welcome.png) | No project open. Two doors, nothing preselected. |
| [`02-new-story---what.png`](screens/02-new-story---what.png) | Novel or rulebook, first, because it decides the checks and the shape of the workspace. |
| [`03-new-manuscript---start.png`](screens/03-new-manuscript---start.png) | **New manuscript.** A way in, not an inventory: a character, a situation, or a question. Nothing else is asked. |
| [`04-existing-manuscript---what-i-read.png`](screens/04-existing-manuscript---what-i-read.png) | **Existing manuscript.** The file is read first — 41,200 words, 24 scenes, 3 viewpoints — and the questions are produced *from what was found*: which name, which viewpoint, is that thread intentional. |
| [`05-first-session---collaborator.png`](screens/05-first-session---collaborator.png) | The collaborator opens with proposals, not questions. The writer pushes back; it answers with the consequence. Nothing is canon until accepted. |
| [`06-workspace---novel.png`](screens/06-workspace---novel.png) | Scenes, the scene, continuity. Dark theme. |
| [`07-workspace---rulebook.png`](screens/07-workspace---rulebook.png) | Book tree, entity fields, cross-references. Light theme. |
| [`08-source-control.png`](screens/08-source-control.png) | Changes, staged, commit, branches, tags, merge conflict. |

## Running the prototype

`canonwright-ux.figcraft` is a **clickable prototype**. Every screen is a frame
and the buttons are wired, so the file can be played: open it in FigCraft and
press play (Alt+9 shows the wiring).

```
00 Set up GitHub ──open the browser / skip──▶ 01 Main
01 Main ──new project──▶ 02 New project        ──new discussion──▶ 04 Project
02 New project ──GitHub repo──▶ 03 Choose a repository
                ──create / cancel──▶ 04 Project / 01 Main
03 Choose a repository ──use / back──▶ 04 Project / 02 New project
04 Project ──manuscript──▶ 05 Manuscript       ──source──▶ 07 Source control
05 / 06 ──discussion / source──▶ 04 / 07      07 ──discussion / manuscript──▶ 04 / 05
```

Nineteen links, checked. `python3 verify_prototype.py` re-checks them.

### The flow it encodes

1. **First run.** If `git` or `gh` is missing, or nobody is signed in, the app
   walks through it: what it detected, what it is about to run, the device code,
   and an honest "skip for now" that leaves local files working and repositories
   unavailable.
2. **Main.** The harness's home screen, with **New project** and **New
   discussion** where it used to say *New session*. Projects and discussions are
   listed at left; the centre says what a project is and what it holds.
3. **New project — setup only.** Name, kind, and four named actions:
   **Start blank**, **Open a file…**, **Open a folder…**, **Connect a
   repository…**. Each button is the action, so there is no separate Create
   step and nothing to explain. No questions about the book: genre, point of
   view, the protagonist and the world are the collaborator's to ask, in
   discussion, afterwards.
4. **Project.** The collaborator's opening discussion — proposals, not a
   questionnaire. This is where the book is actually started.

### Changing it

- **Change the `.figcraft`, not the code.** A layout argument costs minutes here
  and days in React.
- **Every clickable thing must be legible.** Interactions live on the button's
  own rect; the label is a layer above it. An overlay in the button's fill hides
  the label and turns the prototype into a guessing game.
- **No explaining, no hedging.** The interface never spends space on what it is
  *not* doing, what will *not* happen, or why a decision was made. Name the
  control accurately and the behaviour is obvious. A panel headed "NOT ASKED
  HERE" is an admission that the naming failed.

## The screens

| Screen | What it settles |
| --- | --- |
| `00-set-up-github.png` | First run: git and GitHub, the device code, and skipping. |
| `01-main.png` | **New project** and **New discussion**, projects and discussions listed. |
| `02-new-project.png` | Setup only: name, kind, and blank / file / folder / repo. |
| `03-choose-a-repository.png` | Picking the repository that already holds the manuscript. |
| `04-project.png` | The collaborator opens with proposals; nothing is canon until accepted. |
| `05-manuscript---novel.png` | Scenes, the scene, continuity. |
| `06-manuscript---rulebook.png` | Book tree, entity fields, checks. |
| `07-source-control.png` | Changes, commits, branches, tags. |

## The palette

Taken from the logo, so the product and the mark agree:

| | Dark | Light |
| --- | --- | --- |
| Background | `#14161C` | `#F7F3FF` |
| Surface | `#1B1F27` | `#FFFFFF` |
| Raised | `#232833` | `#F1ECFB` |
| Line | `#2E3542` | `#E2DAF0` |
| Text | `#F2EFF9` | `#25243A` |
| Muted | `#9A93AD` | `#6F6885` |
| Accent | `#C5A7FF` | `#8058C5` |

The app's tokens (`--obd-*` in `apps/desktop/ui/src/styles.css`) carry the same
roles, so a screen can be read against the CSS.

## The source, and regenerating

[`canonwright-ux.figcraft`](canonwright-ux.figcraft) is the editable document —
one page per screen. The PNGs are exports of it and the `.figcraft` file is the
source of truth; editing a PNG by hand would be undone by the next export.

```sh
python3 design/build_screens.py    # emit the command list (design/screens.json)
python3 design/build_document.py   # batch it through figcraft's MCP, save, export
```

`build_document.py` talks to `figcraft-cli mcp` over stdio through
[`figcraft_mcp.py`](figcraft_mcp.py), a ~90-line JSON-RPC client. Requires
`figcraft-cli` on `PATH`.

## Rules for changing these

- **Change the `.figcraft` file, not the code.** A layout argument costs minutes
  here and days in React.
- **Both themes are specified.** A screen that only works in dark is half a
  screen.
- **Every screen must answer "what does the writer do next?"** If a panel has
  nothing to say, it should not have width.

## Status

Reference only. Nothing in `apps/desktop` has been rebuilt against these yet;
the gap and the order of work are tracked in the [README](../README.md).
