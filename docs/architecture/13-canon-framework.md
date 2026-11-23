# Canon frameworks

A project's canon is not a fixed schema. It is a **framework**: a small document
that says which areas of canon this project has, and every entry lives in a file
of its own inside the repository.

Two things follow from that, and they are the point:

- The framework is **data in the repo**, so it is diffed, reviewed and restored
  like everything else. A writer who reorganises their characters does not need
  a migration.
- The collaborator can **add, change and remove areas** with tools, because the
  framework is just a document it is allowed to write.

## What is on disk

```
the-salt-road/
├── story.json                     the manuscript: scenes, meta, nothing else
├── canon/
│   ├── framework.json             which areas exist, and in what order
│   ├── characters/
│   │   ├── mara-vale.md
│   │   └── harek-vale.md
│   ├── plot/
│   │   └── act-one.md
│   ├── arcs/
│   │   └── mara-fear-to-command.md
│   └── world-rules/
│       └── no-salt-unsealed.md
├── manuscript/
│   └── scene-03.md
└── .canonwright/
    └── issues/                    local issues; mirrored to GitHub when connected
```

One entry per file. Front matter carries the typed fields, the body carries the
prose, and a `refs:` list carries the links — by id, never by page number, so a
reference cannot rot when the file is renamed.

```markdown
---
id: mara-vale
kind: character
name: Mara Vale
fields:
  pov: true
  want: The salt through the ford
  need: Admit she left the valley out of fear
refs:
  - to: harek-vale
    as: brother
---
Mara has run the valley road for eleven years, and has never once been late.
```

## The framework document

```json
{
  "framework": "novel",
  "version": 1,
  "areas": [
    { "id": "characters", "label": "Characters", "kind": "entry", "order": 10 },
    { "id": "plot",       "label": "Plot",       "kind": "entry", "order": 20 },
    { "id": "arcs",       "label": "Arcs",       "kind": "entry", "order": 30 },
    { "id": "motivations","label": "Motivations","kind": "entry", "order": 40 },
    { "id": "goals",      "label": "Goals",      "kind": "entry", "order": 50 },
    { "id": "threads",    "label": "Threads",    "kind": "entry", "order": 60 },
    { "id": "world-rules","label": "World rules","kind": "entry", "order": 70 }
  ]
}
```

`kind` is `entry` (one file per item), `list` (one file, one line per item) or
`tree` (nested files, for a rulebook's chapters and sections). `order` decides
the order in the panel.

## Presets

Written into `canon/framework.json` when the project is created, from a preset
compiled into the binary, so a new project has something in the panel
immediately and the file is there to be edited.

**Novel** — Characters, Plot, Arcs, Motivations, Goals, Threads, World rules.

**Rulebook** — Characters, Species, Classes, Rules, Items, Adversaries, Threads.

Presets are a starting point, not a constraint: the collaborator can add an area
the preset lacks ("Ships", "Factions", "Timeline") and remove one it does not
use. An empty area is a prompt, not an error.

## Issues

`Issues` is **not** an area of canon and cannot be added, removed or renamed. It
is part of the system, and it is the one place where the project's own health is
reported: continuity breaks, dangling threads, broken references, incomplete
entries, unresolved imports.

The storage is deliberately not the user's problem. When GitHub is connected,
issues are GitHub issues carrying a `canonwright` label, so they are searchable,
assignable and linkable like any other; when it is not, they live in
`.canonwright/issues/` and sync later. **The interface never says "GitHub
issue".** A writer sees *Issues* with a count, a title, where it came from and a
severity. Git is how the product keeps the manuscript safe, not something the
writer has to know about.

Issues are always advisory: nothing blocks a save, a commit or a scene. The one
exception — a broken reference in a rulebook — is reported as an `error`
severity, which still does not block; it marks the entry as unshippable.

## Tools

The collaborator gets these, and nothing else may write the framework:

| Tool | Effect |
| --- | --- |
| `framework_read` | The areas, their kinds, order and counts |
| `framework_add_area` | Append an area, creating its directory |
| `framework_update_area` | Rename, reorder, change `kind` |
| `framework_remove_area` | Remove an area; refuses `issues`, and refuses a non-empty area unless `force` is set |
| `canon_read` / `canon_write` / `canon_delete` | Entries, validated against the framework and the reference graph |
| `issue_list` / `issue_create` / `issue_close` | The fixed surface |

Removal is the dangerous one: deleting an area deletes its files, which is why
it goes through the same commit path as every other change and appears in the
diff before it is committed.

## Why files rather than a database

- The diff is the review. A writer can see that Harek became Mara's brother in
  one line of front matter.
- A broken build cannot eat the canon: the files are the canon.
- The framework is readable and hand-editable, so the collaborator is not the
  only thing that can create an area.
- Nothing here needs a migration when the shapes change; `version` is enough.

The projection into SQLite that the desktop app uses for search is a cache. It
can be rebuilt from the files at any time, and is.
