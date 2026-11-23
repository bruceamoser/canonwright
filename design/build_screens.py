#!/usr/bin/env python3
"""Generate Canonwright's prototype in FigCraft.

    python3 design/build_screens.py     # emit the command list
    python3 design/build_document.py    # batch it through figcraft's MCP, save, export

The flow the prototype follows, and the reason for each screen:

    00  Set up GitHub          first run, and the only integration the product needs
    01  Main                   projects + "New project" / "New discussion"
    02  New project            setup only: name, kind, what it starts from
    03  Choose a repository    picking a repo, when that is the source
    04  Project                the collaborator's opening discussion
    05  Manuscript             scenes, the scene, continuity
    06  Rulebook               book tree, entities, references
    07  Source control         changes, commits, branches, tags

Project creation asks nothing about the manuscript. It configures the project;
the collaborator asks about the book afterwards, in discussion.
"""

import json
from pathlib import Path

FONT = "DejaVu Sans"
MONO = "DejaVu Sans Mono"

DARK = dict(
    bg="#14161C", surface="#1B1F27", raised="#232833", line="#2E3542",
    text="#F2EFF9", body="#CFC9DE", muted="#9A93AD", subtle="#6F6885",
    accent="#C5A7FF", accent2="#8058C5", on_accent="#25243A",
    good="#7BD88F", warn="#F2C879", bad="#FF8A8A",
)
LIGHT = dict(
    bg="#F7F3FF", surface="#FFFFFF", raised="#F1ECFB", line="#E2DAF0",
    text="#25243A", body="#4A4360", muted="#6F6885", subtle="#8B84A0",
    accent="#8058C5", accent2="#C5A7FF", on_accent="#FFFFFF",
    good="#3F8F55", warn="#9A6B12", bad="#B4443C",
)

commands = []
frames = []
CURRENT = {"ref": None, "name": ""}


def cmd(command, as_=None, **params):
    step = {"command": command, "params": params}
    if as_ is not None:
        step["as"] = as_
    commands.append(step)


def screen(name, palette):
    """One page, one frame. The frame is what a prototype interaction navigates to."""
    ref = f"screen:{name}"
    frames.append({"command": "page.add", "params": {"name": name}})
    frames.append({
        "command": "page.select",
        "params": {"index": sum(1 for step in frames if step["command"] == "page.add")},
    })
    frames.append({
        "command": "frame.create",
        "as": ref,
        "params": {"x": 0, "y": 0, "width": 1440, "height": 900, "name": name,
                   "props": {"fill": palette["bg"]}},
    })
    CURRENT["ref"] = ref
    CURRENT["name"] = name


def link(element_as, target_name):
    commands.append({
        "command": "proto.interaction.add",
        "params": {
            "id": {"$ref": element_as},
            "trigger": {"type": "onClick"},
            "action": {"type": "navigate", "dest": {"$ref": f"screen:{target_name}"}},
        },
    })


def rect(x, y, w, h, fill, radius=0, stroke=None, name=None, as_=None):
    props = {"fill": fill, "cornerRadius": radius}
    if stroke:
        props["stroke"] = stroke
        props["strokeWeight"] = 1
    params = {"x": x, "y": y, "width": w, "height": h, "name": name or "rect", "props": props}
    if CURRENT["ref"]:
        params["parent"] = {"$ref": CURRENT["ref"]}
    cmd("shape.rectangle", as_=as_, **params)


def text(x, y, s, size, color, width=None, bold=False, mono=False, name=None, as_=None):
    props = {"fontSize": size, "fontFamily": MONO if mono else FONT, "fill": color}
    if bold:
        props["fontStyle"] = "Bold"
    params = {"text": s, "x": x, "y": y, "props": props, "name": name or s[:26]}
    if width:
        params["width"] = width
    if CURRENT["ref"]:
        params["parent"] = {"$ref": CURRENT["ref"]}
    cmd("text.create", as_=as_, **params)


def button(x, y, w, h, label, palette, target=None, as_=None, tone="primary", size=12):
    """A button as one rect plus one label. The rect is what navigates."""
    if tone == "primary":
        fill, ink = palette["accent"], palette["on_accent"]
    elif tone == "quiet":
        fill, ink = palette["surface"], palette["muted"]
    else:
        fill, ink = palette["raised"], palette["body"]
    # Step names must be unique across the batch, so they carry their screen.
    handle = f"{CURRENT['name']}:{as_}" if as_ else None
    rect(x, y, w, h, fill, radius=8,
         stroke=None if tone == "primary" else palette["line"], as_=handle, name=label)
    text(x + 14, y + h / 2 - size * 0.62, label, size, ink, bold=True)
    if handle and target:
        link(handle, target)


def chip(x, y, label, palette, tone="muted", w=None):
    colour = {"muted": palette["muted"], "accent": palette["accent"], "good": palette["good"],
              "warn": palette["warn"], "bad": palette["bad"]}[tone]
    width = w or (len(label) * 6.6 + 18)
    rect(x, y, width, 20, palette["raised"], radius=10, stroke=palette["line"])
    text(x + 9, y + 5, label, 10, colour, width=width - 12)
    return width


def app_frame(palette, title):
    """Title bar shared by every screen: the mark, the title, and Settings."""
    rect(0, 0, 1440, 40, palette["surface"], name="titlebar")
    rect(0, 39, 1440, 1, palette["line"], name="titlebar rule")
    rect(16, 10, 20, 20, palette["accent2"], radius=6, name="mark")
    text(44, 13, "Canonwright", 11, palette["text"], bold=True)
    rect(118, 11, 1, 18, palette["line"])
    text(132, 13, title, 10, palette["muted"])
    text(1352, 13, "Settings", 10, palette["muted"])


# ------------------------------------------------------------------ 00 github

def setup_github():
    p = DARK
    screen("00 Set up GitHub", p)
    app_frame(p, "Sign in")

    text(120, 104, "Sign in to GitHub", 28, p["text"], bold=True)

    rect(120, 176, 560, 168, p["surface"], radius=12, stroke=p["line"])
    text(144, 196, "CODE", 9, p["muted"])
    text(144, 220, "ABCD-1234", 26, p["accent"], bold=True, mono=True)
    text(144, 262, "1  Copy the code", 10, p["body"])
    text(144, 282, "2  Approve at github.com/login/device", 10, p["body"])
    text(144, 310, "git 2.43.0    gh 2.97.0", 9, p["muted"], mono=True)

    button(120, 372, 240, 42, "Sign in with GitHub", p, "01 Main", "go:sign-in", "primary", 12)
    button(376, 372, 200, 42, "Continue without", p, "01 Main", "go:without", "quiet", 11)

    rect(760, 176, 520, 238, p["surface"], radius=12, stroke=p["line"])
    text(784, 196, "SIGNED IN AS", 9, p["muted"])
    text(784, 222, "Not signed in", 12, p["body"])
    rect(784, 254, 472, 1, p["line"])
    text(784, 270, "PERMISSIONS", 9, p["muted"])
    text(784, 294, "read and write repositories", 10, p["body"])
    text(784, 314, "nothing else", 10, p["muted"])

def main_screen():
    p = LIGHT
    screen("01 Main", p)
    app_frame(p, "")

    rect(0, 40, 300, 860, p["surface"], name="sidebar")
    rect(300, 40, 1, 860, p["line"])

    button(20, 64, 260, 40, "New project", p, "02 New project", "go:new-project", "primary", 12)
    button(20, 112, 260, 36, "New discussion", p, "04 Project", "go:new-discussion", "raise", 11)

    text(20, 176, "PROJECTS", 9, p["muted"])
    projects = [("The Salt Road", "Novel · 3 scenes", True), ("Deeds of Legend", "Rulebook · 118 entries", False)]
    for i, (name, meta, active) in enumerate(projects):
        y = 200 + i * 52
        if active:
            rect(12, y - 8, 276, 46, p["raised"], radius=8)
            rect(12, y - 8, 3, 46, p["accent"], radius=2)
        text(28, y, name, 12, p["text"] if active else p["body"], bold=active)
        text(28, y + 17, meta, 9, p["muted"])

    text(20, 320, "DISCUSSIONS", 9, p["muted"])
    for i, (name, when) in enumerate([
        ("Scene 3: the ford", "2h ago"),
        ("Harek's reason", "yesterday"),
        ("Chapter order", "3d ago"),
    ]):
        y = 344 + i * 44
        text(28, y, name, 11, p["body"])
        text(28, y + 16, when, 9, p["subtle"])

    rect(20, 820, 260, 1, p["line"])
    text(20, 834, "GitHub: bruceamoser", 9, p["muted"])
    text(20, 852, "Local model key: configured", 9, p["muted"])

    text(360, 120, "The Salt Road", 24, p["text"], bold=True)
    chip(360, 160, "Novel", p, "accent", w=64)
    chip(432, 160, "third limited · past", p, "muted", w=150)
    chip(590, 160, "3 scenes drafted", p, "muted", w=130)

    rect(360, 224, 700, 200, p["surface"], radius=14, stroke=p["line"])
    button(392, 300, 200, 40, "New project", p, "02 New project", "empty:new-project", "primary", 12)
    button(608, 300, 200, 40, "New discussion", p, "04 Project", "empty:new-discussion", "raise", 11)
    text(392, 364, "⌘⇧P   project        ⌘N   discussion", 9, p["muted"])

    text(360, 520, "IN THIS PROJECT", 9, p["muted"])
    for i, (label, value) in enumerate([
        ("Manuscript", "3 scenes · 4,100 words"),
        ("Canon", "3 characters · 2 facts · 1 thread"),
        ("Source", "main · clean"),
    ]):
        y = 544 + i * 34
        rect(360, y, 700, 1, p["line"])
        text(360, y + 8, label, 10, p["muted"])
        text(920, y + 8, value, 10, p["text"])


# -------------------------------------------------------------- 02 new project

def new_project():
    p = LIGHT
    screen("02 New project", p)
    app_frame(p, "New project")

    text(160, 92, "New project", 26, p["text"], bold=True)

    text(160, 156, "NAME", 9, p["muted"])
    rect(160, 176, 520, 36, p["surface"], radius=8, stroke=p["line"])
    text(176, 187, "The Salt Road", 13, p["text"])

    text(160, 240, "TYPE", 9, p["muted"])
    chip(160, 262, "Novel", p, "accent", w=76)
    chip(246, 262, "Rulebook", p, "muted", w=88)

    text(160, 314, "START FROM", 9, p["muted"])
    actions = [
        ("Start blank", "Blank", "04 Project", "blank"),
        ("Open a file…", "One file", "04 Project", "file"),
        ("Open a folder…", "A folder", "04 Project", "folder"),
        ("Connect a repository…", "GitHub repo", "03 Choose a repository", "repo"),
    ]
    for i, (label, kind, target, handle) in enumerate(actions):
        y = 340 + i * 74
        rect(160, y, 640, 58, p["surface"], radius=10, stroke=p["line"],
             as_=f"{CURRENT['name']}:{handle}", name=label)
        text(184, y + 12, label, 13, p["text"], bold=True)
        text(184, y + 33, kind, 9, p["muted"])
        text(766, y + 20, "→", 13, p["muted"])
        link(f"{CURRENT['name']}:{handle}", target)

    text(160, 652, "LOCATION", 9, p["muted"])
    text(160, 672, "~/Canonwright/workspaces/the-salt-road", 11, p["body"], mono=True)

    button(160, 780, 140, 38, "Cancel", p, "01 Main", "go:cancel", "quiet", 11)


def choose_repository():
    p = LIGHT
    screen("03 Choose a repository", p)
    app_frame(p, "Connect a repository")

    text(160, 92, "Connect a repository", 26, p["text"], bold=True)

    rect(160, 156, 700, 36, p["surface"], radius=8, stroke=p["line"])
    text(176, 167, "Filter repositories", 11, p["subtle"])

    repos = [
        ("bruceamoser/the-salt-road", "private · updated 2h ago"),
        ("bruceamoser/deeds-of-legend", "private · updated yesterday"),
        ("bruceamoser/neon-relic", "public · updated 3d ago"),
        ("bruceamoser/heroes-of-legend", "private · updated 1w ago"),
    ]
    for i, (name, meta) in enumerate(repos):
        y = 212 + i * 64
        selected = i == 0
        rect(160, y, 700, 56, p["surface"], radius=10,
             stroke=p["accent"] if selected else p["line"])
        text(182, y + 11, name, 12, p["text"], bold=selected, mono=True)
        text(182, y + 32, meta, 9, p["muted"])
        if selected:
            text(806, y + 19, "✓", 12, p["accent"])

    button(160, 700, 140, 38, "Back", p, "02 New project", "repo:back", "quiet", 11)
    button(700, 700, 200, 38, "Connect repository", p, "04 Project", "repo:use", "primary", 11)


def right_panel(p, sections, issues, footer_action, footer_target):
    """The canon inspector: labelled areas of canon, then the issue list.

    The areas come from the project's framework, so this panel is the framework
    made visible — add an area and it appears here with its count. Issues is
    fixed: always present, never removable.
    """
    x, w = 1120, 320
    rect(x, 40, w, 860, p["surface"], name="canon panel")
    rect(x, 40, 1, 860, p["line"])

    rect(x + 16, 56, 92, 24, p["raised"], radius=6)
    text(x + 30, 62, "Canon", 10, p["text"], bold=True)
    text(x + 124, 62, f"Issues {len(issues)}", 10, p["muted"])
    rect(x + 16, 86, w - 32, 1, p["line"])

    y = 100
    for label, rows in sections:
        rect(x + 16, y, w - 32, 18, p["raised"], radius=4)
        text(x + 26, y + 3, label.upper(), 9, p["muted"])
        text(x + w - 62, y + 3, str(len(rows)), 9, p["subtle"])
        text(x + w - 30, y + 2, "+", 11, p["muted"])
        y += 18
        # Two rows per area: the panel is an index, not the table.
        for name, meta in rows[:2]:
            text(x + 30, y + 4, name, 9, p["text"], width=210)
            if meta and meta != "—":
                text(x + w - 92, y + 5, meta, 8, p["subtle"], width=76)
            y += 17
        y += 14

    rect(x + 16, y, w - 32, 1, p["line"])
    y += 12
    text(x + 16, y, "ISSUES", 9, p["muted"])
    y += 20
    for label, where, tone in issues:
        text(x + 18, y, "●", 9, p[tone])
        text(x + 34, y - 1, label, 9, p["text"], width=250)
        text(x + 34, y + 12, where, 8, p["subtle"], width=250)
        y += 34

    button(x + 16, 790, w - 32, 34, footer_action, p, footer_target,
           "panel:footer", "primary", 10)


def project_discussion():
    p = DARK
    screen("04 Project", p)
    app_frame(p, "The Salt Road")

    rect(0, 40, 300, 860, p["surface"], name="sidebar")
    rect(300, 40, 1, 860, p["line"])
    text(20, 64, "THE SALT ROAD", 9, p["muted"])
    text(20, 84, "Novel", 10, p["body"])
    for i, (label, active) in enumerate([
        ("Discussion", True), ("Manuscript", False), ("Canon", False), ("Source", False),
    ]):
        y = 124 + i * 34
        if active:
            rect(12, y - 8, 276, 30, p["raised"], radius=6)
            text(28, y, label, 11, p["text"], bold=True)
        else:
            text(28, y, label, 11, p["muted"])
    text(20, 300, "DISCUSSIONS", 9, p["muted"])
    for i, name in enumerate(["Scene 3: the ford", "Harek's reason"]):
        text(28, 324 + i * 26, name, 10, p["body"])

    text(336, 64, "DISCUSSION", 9, p["muted"])
    rect(336, 88, 780, 236, p["surface"], radius=12, stroke=p["line"])
    text(358, 104, "Collaborator", 11, p["accent"], bold=True)
    text(358, 128, "Here is the shape I would propose for a third-limited fantasy about a", 12, p["text"], width=740)
    text(358, 146, "closed road:", 12, p["text"], width=740)
    text(358, 170, "· Mara Vale, a caravan master. Wants the salt through the ford;", 11, p["body"], width=740)
    text(358, 188, "   needs to admit she left the valley out of fear.", 11, p["body"], width=740)
    text(358, 206, "· Harek, the ford warden, being paid to keep it shut.", 11, p["body"], width=740)
    text(358, 224, "A hard rule: no salt crosses without the duke's seal.", 11, p["body"], width=740)
    text(358, 252, "Two ways in. Which?", 11, p["muted"], width=740)

    # the proposals, each with its own action
    for i, (label, target) in enumerate([
        ("Open on the ford already shut", "05 Manuscript — novel"),
        ("Open as it shuts, in front of her", "05 Manuscript — novel"),
    ]):
        y = 264 + i * 28
        rect(358, y, 460, 24, p["raised"], radius=6, stroke=p["line"],
             as_=f"{CURRENT['name']}:option-{i}", name=label)
        text(370, y + 5, label, 10, p["text"])
        link(f"{CURRENT['name']}:option-{i}", target)

    rect(336, 344, 780, 46, p["raised"], radius=10, stroke=p["line"])
    text(358, 358, "Make Harek her brother, and give him the better reason.", 11, p["text"], width=740)

    rect(336, 408, 780, 128, p["surface"], radius=12, stroke=p["line"])
    text(358, 424, "Collaborator", 11, p["accent"], bold=True)
    text(358, 448, "Brother changes the first scene: the ford is a family argument before it is", 12, p["text"], width=740)
    text(358, 466, "a political one. I added the relationship and one fact — Harek is owed money", 12, p["text"], width=740)
    text(358, 484, "he cannot repay.", 12, p["text"], width=740)
    text(358, 508, "Harek Vale · brother of Mara · owed 200 marks", 10, p["muted"], width=740)

    rect(336, 560, 780, 40, p["raised"], radius=10, stroke=p["line"])
    text(350, 572, "Reply", 11, p["subtle"])

    sections = [
        ("Characters", [("Mara Vale", "POV"), ("Harek Vale", "POV"), ("Ilse", "—")]),
        ("Plot", [("Act one", "4 scenes"), ("Act two", "6 scenes")]),
        ("Arcs", [("Mara: fear → command", "open"), ("Harek: debt → ruin", "open")]),
        ("Motivations", [("Mara wants the ford", "—"), ("Harek needs money", "—")]),
        ("Goals", [("Reach the duke by spring", "—")]),
        ("Threads", [("The forged seal", "planted")]),
        ("World rules", [("No salt crosses unsealed", "hard")]),
    ]
    issues = [
        ("Scene 4 has no cause", "continuity · advisory", "warn"),
        ("Forged seal unresolved", "thread · advisory", "warn"),
    ]
    right_panel(p, sections, issues, "Open the manuscript", "05 Manuscript — novel")

def project_rail(p, name, subtitle, active):
    rect(0, 40, 300, 860, p["surface"], name="sidebar")
    rect(300, 40, 1, 860, p["line"])
    text(20, 64, name.upper(), 9, p["muted"])
    text(20, 84, subtitle, 10, p["body"])
    for i, label in enumerate(["Discussion", "Manuscript", "Canon", "Source"]):
        y = 124 + i * 34
        if label == active:
            rect(12, y - 8, 276, 30, p["raised"], radius=6)
            text(28, y, label, 11, p["text"], bold=True)
        else:
            text(28, y, label, 11, p["muted"])


# ------------------------------------------------------------ 05 novel workspace
def novel_workspace():
    p = DARK
    screen("05 Manuscript — novel", p)
    app_frame(p, "The Salt Road")
    project_rail(p, "The Salt Road", "Novel · 3 scenes", "Manuscript")

    text(336, 64, "SCENES", 9, p["muted"])
    for i, (n, title, status, tone) in enumerate([
        ("1", "The ford", "canon", "good"),
        ("2", "The tally", "canon", "good"),
        ("3", "Ash on the water", "drafted", "accent"),
        ("4", "The seal", "planned", "muted"),
    ]):
        y = 92 + i * 46
        if i == 2:
            rect(336, y - 8, 300, 40, p["raised"], radius=8)
            rect(336, y - 8, 3, 40, p["accent"], radius=2)
        text(352, y, n, 10, p["muted"], mono=True)
        text(372, y - 2, title, 12, p["text"] if i == 2 else p["body"], bold=(i == 2))
        text(372, y + 15, status, 9, p[tone])

    text(668, 64, "SCENE 3", 9, p["muted"])
    text(668, 84, "Ash on the water", 24, p["text"], bold=True)
    chip(668, 120, "drafted", p, "accent", w=62)
    chip(738, 120, "pov: Mara", p, "muted", w=74)
    chip(820, 120, "suspicion → certainty", p, "muted", w=150)
    for i in range(8):
        width = [420, 400, 412, 300, 418, 396, 250, 410][i]
        rect(668, 164 + i * 28, width, 8, p["raised"], radius=4)

    sections = [
        ("Characters", [("Mara Vale", "POV"), ("Harek Vale", "POV"), ("Ilse", "—")]),
        ("Plot", [("Act one", "4 scenes"), ("Act two", "6 scenes")]),
        ("Arcs", [("Mara: fear → command", "open"), ("Harek: debt → ruin", "open")]),
        ("Motivations", [("Mara wants the ford", "—"), ("Harek needs money", "—")]),
        ("Goals", [("Reach the duke by spring", "—")]),
        ("Threads", [("The forged seal", "planted")]),
        ("World rules", [("No salt crosses unsealed", "hard")]),
    ]
    issues = [
        ("Scene 4 has no cause", "continuity · advisory", "warn"),
        ("Forged seal unresolved", "thread · advisory", "warn"),
    ]
    right_panel(p, sections, issues, "Open the manuscript", "05 Manuscript — novel")

def rulebook_workspace():
    p = LIGHT
    screen("06 Manuscript — rulebook", p)
    app_frame(p, "Deeds of Legend")
    project_rail(p, "Deeds of Legend", "Rulebook · 118 entries", "Manuscript")

    text(336, 64, "BOOK", 9, p["muted"])
    for i, (label, active) in enumerate([
        ("1 · The core mechanic", False), ("2 · Characters", True), ("    Classes", False),
        ("    Species", False), ("3 · Adversaries", False), ("4 · Equipment", False),
    ]):
        y = 92 + i * 30
        if active:
            rect(336, y - 8, 300, 26, p["raised"], radius=6)
        text(352, y, label, 11, p["text"] if active else p["body"], bold=active)

    text(668, 64, "CLASS · 24 of 118", 9, p["muted"])
    text(668, 84, "Fighter", 24, p["text"], bold=True)
    text(668, 120, "A trained combatant who holds the line.", 11, p["muted"])
    for i, (key, value) in enumerate([
        ("Traits", "Might, Edge"),
        ("Hit points", "8 + Might at 1st level"),
        ("Starting gear", "One martial weapon, a shield"),
        ("Advancement", "A talent every odd level"),
    ]):
        y = 160 + i * 56
        text(668, y, key, 9, p["muted"])
        rect(668, y + 14, 400, 30, p["surface"], radius=6, stroke=p["line"])
        text(678, y + 23, value, 11, p["text"])

    sections = [
        ("Characters", [("Fighter", "class"), ("Duelist", "class"), ("Mage", "class")]),
        ("Species", [("Human", "—"), ("Elf", "—"), ("Dwarf", "—")]),
        ("Rules", [("Core resolution", "2d10"), ("Advancement", "—")]),
        ("Items", [("Longsword", "weapon"), ("Rations", "gear")]),
        ("Adversaries", [("Bandit", "tier 1"), ("Wyrm", "tier 4")]),
        ("Threads", [("The broken oath", "planted")]),
    ]
    issues = [
        ("grapple is undefined", "reference · error", "bad"),
        ("Duelist has no fields", "entry · advisory", "warn"),
        ("Bandit has no attacks", "entry · error", "bad"),
    ]
    right_panel(p, sections, issues, "Open the manuscript", "06 Manuscript — rulebook")

setup_github()
main_screen()
new_project()
choose_repository()
project_discussion()
novel_workspace()
rulebook_workspace()

# Remove the empty page the document starts with, so page index == screen index.
cmd("page.delete", id=1)

out = Path(__file__).with_name("screens.json")
deletes = [step for step in commands if step["command"] == "page.delete"]
body = [step for step in commands if step["command"] != "page.delete"]
out.write_text(json.dumps(frames + body + deletes, indent=1))
print(f"{len(frames)} frame steps + {len(body)} content steps → {out}")
