# Changelog

## 0.2.0

The first release where Canonwright behaves like a product rather than a harness
with a writing layer bolted on. Two changes matter.

### The app opens on its own front door

Until a project is opened, the window shows Canonwright's screens, not the
agent harness:

- **Sign in** — a token, checked against GitHub and kept in the OS keychain.
  Skippable; local files and folders still work without it.
- **Main** — projects on the left, and the two actions that start anything:
  **New project** and **New discussion**.
- **New project** — a name, novel or rulebook, and four actions that are the
  actions: **Start blank**, **Open a file…**, **Open a folder…**, **Connect a
  repository…**

`enteredProject` gates the handover, so a restored session can no longer drop
you into a project you did not choose. That was the old first-run bug.

### GitHub without the CLI

The app no longer shells out to `gh`. It reads a token from the keychain and
speaks REST:

| Was | Now |
| --- | --- |
| `gh api user` | `GET /user` |
| `gh repo list --json` | `GET /user/repos` |
| `gh repo clone` | `git clone`, then the token is stripped from the remote |
| `gh auth login --web` | a token, verified before it is stored |

Auth state is per-app rather than per-machine: signing in here does not change
which account any other tool sees, and signing out does not affect `gh`. If a
token is rejected, nothing is stored, so a typo cannot leave the app "signed in"
to nothing.

`git` remains required — for clone, commit and push — and is the only external
tool the app needs.

### Also in this release

- Seven-screen clickable prototype in `design/` (`canonwright-ux.figcraft`),
  with `verify_prototype.py` checking that every link still resolves.
- `docs/architecture/13-canon-framework.md`: canon as repo files — a framework
  document naming the areas (Characters, Plot, Arcs, Motivations, Goals,
  Threads, World rules), one file per entry, references by id, and an `Issues`
  surface that cannot be removed. **Design only** — the panel is not built yet.
- The desktop app's 86 tests pass; clippy is clean.

### Known gaps

- Opening a project still shows the inherited harness view. The discussion
  screen with the canon inspector is the next release.
- Canon areas, areas' entries and Issues exist as a specification, not as code.
- Sign-in is a token. The OAuth device flow needs an app registration and is not
  wired up.

## 0.1.0

First installable build: the desktop app, the story state graph, manuscript
import, lint tickets, and the release pipeline for Linux, macOS and Windows.
