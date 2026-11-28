# Changelog

## 0.2.30

Chat, Writer and Editorial Reviewer can assess English manuscript passages locally. Quick screening uses Desklib. Strict generation checks require Desklib and Falcon Binoculars agreement. Legacy GPT-2 follow-up remains available for comparison. Optional research modes rank passages by full-size Fast-DetectGPT discrepancy or EditLens editing intensity. Reports include source offsets and excerpts. Scores cannot establish authorship.

The Linux Debian package includes setup scripts. Model weights are separate downloads. EditLens requires authorized access to Pangram’s noncommercial checkpoint. See the detector README for setup and interpretation.

## 0.2.1

The release that makes the previous one visible.

### The updater could not see 0.2.0

`GET /repos/{repo}/releases/latest` was called without a token. On a private
repository GitHub answers `404` to anonymous callers — the same status it uses
for "no release" — and the updater read that as *up to date*. So 0.2.0 shipped,
was marked latest, and stayed invisible.

The check now sends the token from the keychain, and an anonymous `404` is
reported as **needing a token** instead of claiming the app is current. An
authenticated `404` still means "no release", which is now the only thing it can
mean.

Consequence worth stating: **checking for updates requires being signed in.**
That is unavoidable while the repository is private — an anonymous request
cannot see the release list at all. If the repository is ever made public, the
anonymous path starts working again with no code change.

### Also

- One test now covers both meanings of `404`; the previous one asserted the
  behaviour that caused this.

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
