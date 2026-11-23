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
    app_frame(p, "First run")

    text(120, 96, "First, GitHub.", 30, p["text"], bold=True)
    text(120, 140, "Canonwright keeps the manuscript in a git repository: that is how a draft has", 12, p["body"])
    text(120, 158, "history, a backup, and a version you can hand to someone.", 12, p["body"])

    steps = [
        ("git installed", "2.43.0", "good"),
        ("GitHub CLI installed", "2.97.0", "good"),
        ("Sign in to GitHub", "waiting for you", "accent"),
        ("Ready", "", "muted"),
    ]
    for i, (label, state, tone) in enumerate(steps):
        y = 216 + i * 52
        active = tone == "accent"
        if active:
            rect(120, y - 8, 560, 44, p["raised"], radius=8)
            rect(120, y - 8, 3, 44, p["accent"], radius=2)
        dot = p[tone] if tone != "muted" else p["subtle"]
        text(140, y, "●", 11, dot)
        text(162, y - 2, label, 12, p["text"] if active else p["body"], bold=active)
        if state:
            text(162, y + 15, state, 9, p["muted"])

    rect(120, 440, 560, 132, p["surface"], radius=12, stroke=p["accent"])
    text(144, 456, "SIGN IN", 9, p["accent"])
    text(144, 478, "Enter this code at github.com/login/device:", 11, p["body"])
    rect(144, 500, 190, 40, p["raised"], radius=8, stroke=p["line"])
    text(166, 510, "ABCD-1234", 20, p["accent"], bold=True, mono=True)
    text(144, 548, "This window updates by itself once you approve.", 9, p["muted"])

    button(120, 596, 220, 40, "Open the browser", p, "01 Main", "go:browser", "primary", 12)
    button(356, 596, 160, 40, "Skip for now", p, "01 Main", "go:skip", "quiet", 11)
    text(120, 656, "Skipping is fine: a local file or folder still opens. Repositories stay unavailable until you connect.", 9, p["muted"], width=560)

    rect(760, 216, 520, 300, p["surface"], radius=12, stroke=p["line"])
    text(784, 236, "WHAT CANONWRIGHT NEEDS FROM GITHUB", 9, p["muted"])
    for i, line in enumerate([
        "a repository to hold the manuscript",
        "permission to read it, and to push when you ask",
        "nothing else — no profile, no followers, no feed",
    ]):
        text(784, 266 + i * 24, f"·  {line}", 11, p["body"], width=470)
    rect(784, 344, 472, 1, p["line"])
    text(784, 360, "The GitHub CLI is used rather than a bespoke login: it is already", 10, p["muted"], width=470)
    text(784, 378, "installed on most machines that have git, and it keeps the token", 10, p["muted"], width=470)
    text(784, 396, "in the system keyring rather than in a file this app owns.", 10, p["muted"], width=470)
    text(784, 424, "If gh is missing, this screen offers to install it first, and says", 10, p["muted"], width=470)
    text(784, 442, "which command it will run.", 10, p["muted"], width=470)


# -------------------------------------------------------------------- 01 main
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

    rect(360, 224, 700, 260, p["surface"], radius=14, stroke=p["line"])
    text(392, 256, "Start a project, or pick up a discussion.", 15, p["text"], bold=True)
    text(392, 288, "A project is a manuscript and its canon. A discussion is a conversation", 11, p["body"], width=620)
    text(392, 306, "about it — with the collaborator, about a scene, a character, a rule.", 11, p["body"], width=620)
    button(392, 340, 200, 40, "New project", p, "02 New project", "empty:new-project", "primary", 12)
    button(608, 340, 200, 40, "New discussion", p, "04 Project", "empty:new-discussion", "raise", 11)
    text(392, 404, "⌘⇧P for a project · ⌘N for a discussion", 9, p["muted"])

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

    text(160, 88, "New project", 26, p["text"], bold=True)
    text(160, 126, "This creates a workspace and nothing else. What the book is about is a conversation", 12, p["muted"])
    text(160, 144, "the project has with you afterwards.", 12, p["muted"])

    text(160, 196, "NAME", 9, p["muted"])
    rect(160, 216, 520, 34, p["surface"], radius=8, stroke=p["line"])
    text(174, 226, "The Salt Road", 12, p["text"])
    chip(716, 222, "Novel", p, "accent", w=64)
    chip(790, 222, "Rulebook", p, "muted", w=76)

    text(160, 286, "WHAT ARE YOU STARTING FROM?", 9, p["muted"])
    sources = [
        ("Blank", "No manuscript yet.", False),
        ("One file", "docx, md, epub, pdf.", False),
        ("A folder", "A directory of drafts.", False),
        ("GitHub repo", "An existing repository.", True),
    ]
    for i, (label, hint, selected) in enumerate(sources):
        x = 160 + i * 288
        rect(x, 308, 264, 96, p["surface"], radius=12,
             stroke=p["accent"] if selected else p["line"],
             as_=f"{CURRENT['name']}:source-github" if selected else None, name=label)
        text(x + 20, 326, label, 14, p["text"], bold=True)
        text(x + 20, 352, hint, 10, p["muted"], width=224)
        if selected:
            chip(x + 20, 372, "choose", p, "accent", w=64)
    # The single-file and folder sources open a native picker rather than a
    # screen, so only the repository source has somewhere to go in the prototype.
    link(f"{CURRENT['name']}:source-github", "03 Choose a repository")

    rect(160, 428, 1090, 120, p["raised"], radius=12, stroke=p["line"])
    text(184, 444, "SOURCE", 9, p["warn"])
    text(184, 466, "Choose one of the four above. Nothing is read until the project exists —", 11, p["text"], width=1040)
    text(184, 484, "the import runs as the project's first action, so a half-read file never leaves a broken workspace.", 11, p["text"], width=1040)
    text(184, 512, "A single file may be .docx, .md, .epub or .pdf. A folder is scanned for the same. A repository is cloned first.", 10, p["muted"], width=1040)

    text(160, 578, "LOCATION", 9, p["muted"])
    text(160, 598, "~/Canonwright/workspaces/the-salt-road", 11, p["muted"], mono=True)

    rect(160, 640, 1090, 1, p["line"])
    text(160, 660, "NOT ASKED HERE", 9, p["muted"])
    for i, line in enumerate([
        "genre, tone, point of view, tense — the collaborator asks in discussion",
        "who the protagonist is, what they want, what the book is about — same",
        "classes, ancestries, items, monsters, rules — the collaborator proposes them",
    ]):
        text(160, 684 + i * 22, f"·  {line}", 10, p["muted"], width=1040)

    button(160, 796, 140, 38, "Cancel", p, "01 Main", "go:cancel", "quiet", 11)
    button(1010, 796, 240, 38, "Create project", p, "04 Project", "go:create", "primary", 12)


# --------------------------------------------------------- 03 choose a repo
def choose_repository():
    p = LIGHT
    screen("03 Choose a repository", p)
    app_frame(p, "New project · GitHub")

    text(160, 88, "Choose a repository", 26, p["text"], bold=True)
    text(160, 126, "Repositories you can write to. The manuscript inside is found when the project", 12, p["muted"])
    text(160, 144, "is created — the largest document, skipping readmes and vendored files.", 12, p["muted"])

    rect(160, 192, 700, 34, p["surface"], radius=8, stroke=p["line"])
    text(174, 202, "Filter…", 11, p["subtle"])

    repos = [
        ("bruceamoser/the-salt-road", "private · updated 2h ago"),
        ("bruceamoser/deeds-of-legend", "private · updated yesterday"),
        ("bruceamoser/neon-relic", "public · updated 3d ago"),
        ("bruceamoser/heroes-of-legend", "private · updated 1w ago"),
    ]
    for i, (name, meta) in enumerate(repos):
        y = 246 + i * 62
        selected = i == 0
        rect(160, y, 700, 54, p["surface"], radius=10,
             stroke=p["accent"] if selected else p["line"])
        text(180, y + 10, name, 12, p["text"], bold=selected, mono=True)
        text(180, y + 30, meta, 9, p["muted"])
        if selected:
            text(810, y + 18, "selected", 9, p["accent"])

    button(160, 560, 140, 38, "Back", p, "02 New project", "repo:back", "quiet", 11)
    button(660, 560, 180, 38, "Use repository", p, "04 Project", "repo:use", "primary", 11)


# ------------------------------------------------------------------ 04 project
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
    rect(336, 88, 780, 210, p["surface"], radius=12, stroke=p["line"])
    text(358, 104, "Collaborator", 11, p["accent"], bold=True)
    text(358, 128, "The project is set up, so let me start rather than ask you to.", 12, p["text"], width=740)
    text(358, 152, "Here is the shape I would propose for a third-limited fantasy about a", 12, p["text"], width=740)
    text(358, 170, "closed road:", 12, p["text"], width=740)
    text(358, 194, "· Mara Vale, a caravan master. Wants the salt through the ford;", 11, p["body"], width=740)
    text(358, 212, "   needs to admit she left the valley out of fear.", 11, p["body"], width=740)
    text(358, 230, "· Harek, the ford warden, being paid to keep it shut.", 11, p["body"], width=740)
    text(358, 254, "A hard rule: no salt crosses without the duke's seal.", 11, p["body"], width=740)
    text(358, 276, "Say what to change, or ask me to go further before I plant scene one.", 10, p["muted"], width=740)

    rect(336, 316, 780, 46, p["raised"], radius=10, stroke=p["line"])
    text(358, 330, "Mara is good. Make Harek her brother and give him the better reason.", 11, p["text"], width=740)

    rect(336, 380, 780, 140, p["surface"], radius=12, stroke=p["line"])
    text(358, 396, "Collaborator", 11, p["accent"], bold=True)
    text(358, 420, "Brother changes the first scene: the ford is a family argument before it is", 12, p["text"], width=740)
    text(358, 438, "a political one. I have added the relationship and one fact — Harek is owed", 12, p["text"], width=740)
    text(358, 456, "money he cannot repay — and left the reason open.", 12, p["text"], width=740)
    text(358, 484, "Two ways to open: the ford already shut, or shut while she watches.", 10, p["muted"], width=740)

    rect(336, 540, 780, 90, p["surface"], radius=12, stroke=p["line"])
    text(358, 558, "Tell me who burned the ford, or let me draft scene one and find out.", 11, p["muted"], width=740)
    rect(336, 650, 780, 40, p["raised"], radius=10, stroke=p["line"])
    text(350, 662, "Reply, or press ⌘K for a command…", 11, p["subtle"])

    rect(1136, 40, 304, 860, p["surface"], name="canon")
    rect(1136, 40, 1, 860, p["line"])
    text(1160, 64, "PROPOSED CANON", 9, p["muted"])
    for i, (name, kind) in enumerate([
        ("Mara Vale", "character"), ("Harek", "character"), ("no-salt-unsealed", "hard rule"),
    ]):
        y = 92 + i * 44
        text(1160, y, name, 10, p["text"], mono=True)
        text(1160, y + 15, kind, 9, p["accent"])
    rect(1160, 236, 256, 1, p["line"])
    text(1160, 252, "Nothing is canon until you accept it.", 10, p["muted"], width=240)

    button(1160, 700, 256, 34, "Open the manuscript", p, "05 Manuscript — novel", "go:manuscript", "raise", 10)
    button(1160, 744, 256, 34, "Source control", p, "07 Source control", "go:source", "quiet", 10)


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

    rect(1112, 40, 328, 860, p["surface"], name="continuity")
    rect(1112, 40, 1, 860, p["line"])
    text(1136, 64, "CONTINUITY", 9, p["muted"])
    rect(1136, 88, 280, 84, p["raised"], radius=8, stroke=p["line"])
    text(1152, 100, "[CAUSALITY_BREAK]", 9, p["bad"], mono=True)
    text(1152, 118, "Scene 4 has no link to an earlier scene.", 10, p["text"], width=250)
    text(1152, 148, "advisory", 9, p["muted"])
    rect(1136, 184, 280, 84, p["raised"], radius=8, stroke=p["line"])
    text(1152, 196, "[DANGLING_SETUP]", 9, p["warn"], mono=True)
    text(1152, 214, "The forged seal was planted 6 scenes ago.", 10, p["text"], width=250)
    text(1152, 244, "confirmed against the timeline", 9, p["muted"])

    button(1136, 700, 280, 34, "Discussion", p, "04 Project", "go:discussion", "quiet", 10)
    button(1136, 744, 280, 34, "Source control", p, "07 Source control", "go:source", "raise", 10)


# --------------------------------------------------------- 06 rulebook workspace
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

    rect(1112, 40, 328, 860, p["surface"], name="checks")
    rect(1112, 40, 1, 860, p["line"])
    text(1136, 64, "CHECKS", 9, p["muted"])
    rect(1136, 88, 280, 92, p["raised"], radius=8, stroke=p["bad"])
    text(1152, 100, "[BROKEN_REFERENCE]", 9, p["bad"], mono=True)
    text(1152, 118, "`grapple` is referenced but not defined.", 10, p["text"], width=250)
    text(1152, 148, "error — the book cannot ship with this", 9, p["muted"])
    rect(1136, 192, 280, 84, p["raised"], radius=8, stroke=p["line"])
    text(1152, 204, "[INCOMPLETE_ENTITY]", 9, p["warn"], mono=True)
    text(1152, 222, "Duelist has no fields and no summary.", 10, p["text"], width=250)
    text(1152, 252, "advisory", 9, p["muted"])

    button(1136, 700, 280, 34, "Discussion", p, "04 Project", "go:discussion", "quiet", 10)
    button(1136, 744, 280, 34, "Source control", p, "07 Source control", "go:source", "raise", 10)


# ------------------------------------------------------------- 07 source control
def source_control():
    p = DARK
    screen("07 Source control", p)
    app_frame(p, "The Salt Road · Source")

    rect(0, 40, 300, 860, p["surface"], name="sidebar")
    rect(300, 40, 1, 860, p["line"])
    text(20, 64, "THE SALT ROAD", 9, p["muted"])
    text(20, 84, "Novel", 10, p["body"])
    for i, label in enumerate(["Discussion", "Manuscript", "Canon", "Source"]):
        y = 124 + i * 34
        if label == "Source":
            rect(12, y - 8, 276, 30, p["raised"], radius=6)
            text(28, y, label, 11, p["text"], bold=True)
        else:
            text(28, y, label, 11, p["muted"])

    rect(336, 64, 760, 700, p["surface"], radius=12, stroke=p["line"])
    text(360, 86, "main", 14, p["text"], bold=True)
    text(408, 89, "↑2  ↓0   origin/main", 10, p["muted"])

    text(360, 128, "CHANGES · 3", 9, p["muted"])
    for i, (path, badge, tone) in enumerate([
        ("manuscript/scene-03.md", "M", "warn"),
        ("story.json", "M", "warn"),
        ("manuscript/scene-04.md", "U", "good"),
    ]):
        y = 152 + i * 28
        text(360, y, badge, 9, p[tone], mono=True)
        text(380, y - 1, path, 10, p["text"], mono=True)
        text(1010, y, "stage", 9, p["subtle"])

    text(360, 262, "STAGED · 1", 9, p["muted"])
    text(360, 286, "M", 9, p["good"], mono=True)
    text(380, 285, "story.json", 10, p["text"], mono=True)

    rect(360, 320, 712, 60, p["surface"], radius=8, stroke=p["line"])
    text(372, 334, "Scene 3: Mara works out who burned the ford.", 11, p["muted"])
    button(360, 396, 120, 34, "Commit", p, None, "go:commit", "primary", 11)
    text(492, 406, "1 staged", 9, p["muted"])

    text(360, 460, "BRANCHES", 9, p["muted"])
    for i, (name, current) in enumerate([("main", True), ("draft/act-two", False)]):
        y = 484 + i * 24
        text(360, y, "●" if current else "○", 10, p["accent"] if current else p["muted"])
        text(380, y, name, 10, p["text"] if current else p["body"], mono=True)

    text(360, 556, "TAG A DRAFT", 9, p["muted"])
    rect(360, 578, 260, 30, p["raised"], radius=6, stroke=p["line"])
    text(370, 587, "draft-2", 10, p["subtle"])
    rect(632, 578, 120, 30, p["raised"], radius=6, stroke=p["line"])
    text(650, 587, "Tag & push", 9, p["body"])

    text(360, 644, "RECENT", 9, p["muted"])
    for i, (sha, subject) in enumerate([
        ("a3f21c9", "Scene 3: Mara works out who burned the ford"),
        ("7be04d1", "Scene 2: the tally-house"),
    ]):
        y = 668 + i * 24
        text(360, y, sha, 9, p["muted"], mono=True)
        text(430, y, subject, 10, p["body"])

    rect(1120, 64, 300, 220, p["surface"], radius=12, stroke=p["line"])
    text(1144, 84, "THIS IS THE MANUSCRIPT'S HOME", 9, p["muted"])
    for i, line in enumerate([
        "Every commit is a draft to return to.",
        "Every tag is a version to hand over.",
        "Nothing leaves the machine without",
        "the assistant asking first.",
    ]):
        text(1144, 112 + i * 20, line, 10, p["muted"], width=252)

    button(1120, 700, 300, 34, "Discussion", p, "04 Project", "go:discussion", "quiet", 10)
    button(1120, 744, 300, 34, "Manuscript", p, "05 Manuscript — novel", "go:manuscript", "raise", 10)


setup_github()
main_screen()
new_project()
choose_repository()
project_discussion()
novel_workspace()
rulebook_workspace()
source_control()

# Remove the empty page the document starts with, so page index == screen index.
cmd("page.delete", id=1)

out = Path(__file__).with_name("screens.json")
deletes = [step for step in commands if step["command"] == "page.delete"]
body = [step for step in commands if step["command"] != "page.delete"]
out.write_text(json.dumps(frames + body + deletes, indent=1))
print(f"{len(frames)} frame steps + {len(body)} content steps → {out}")
