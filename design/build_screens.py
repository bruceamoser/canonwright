#!/usr/bin/env python3
"""Generate Canonwright's UX screens as a FigCraft document.

Run:  python3 design/build_screens.py
Then: figcraft-cli run design/screens.json --out design/canonwright-ux.figcraft

Every screen is a page, so `figcraft-cli render --page <name>` gives one PNG per
screen. The palette is taken from the logo: ink #25243A, paper #F7F3FF,
lilac #C5A7FF, violet #8058C5.
"""

import json
from pathlib import Path

FONT = "DejaVu Sans"
MONO = "DejaVu Sans Mono"

DARK = dict(
    bg="#14161C", surface="#1B1F27", raised="#232833", line="#2E3542",
    text="#F2EFF9", muted="#9A93AD", accent="#C5A7FF", accent2="#8058C5",
    ink="#25243A", good="#7BD88F", bad="#FF8A8A", warn="#F2C879",
)
LIGHT = dict(
    bg="#F7F3FF", surface="#FFFFFF", raised="#F1ECFB", line="#E2DAF0",
    text="#25243A", muted="#6F6885", accent="#8058C5", accent2="#C5A7FF",
    ink="#25243A", good="#3F8F55", bad="#B4443C", warn="#9A6B12",
)

commands = []


def cmd(command, **params):
    commands.append({"command": command, "params": params})


def page(name, palette):
    # page.add makes the new page current, so the layers below land on it.
    cmd("page.add", name=name)
    cmd("page.setBackground", color=palette["bg"])
    cmd("shape.rectangle", x=0, y=0, width=1440, height=900,
        name=f"{name} / background", props={"fill": palette["bg"]})


def rect(x, y, w, h, fill, radius=0, stroke=None, name=None, o=1):
    props = {"fill": fill, "cornerRadius": radius}
    if stroke:
        props["stroke"] = stroke
        props["strokeWeight"] = 1
    if o != 1:
        props["opacity"] = o
    cmd("shape.rectangle", x=x, y=y, width=w, height=h,
        name=name or "rect", props=props)


def text(x, y, s, size, color, width=None, bold=False, mono=False, height=None, name=None):
    props = {"fontSize": size, "fontFamily": MONO if mono else FONT, "fill": color}
    if bold:
        props["fontStyle"] = "Bold"
    if height:
        props["lineHeight"] = height
    params = {"text": s, "x": x, "y": y, "props": props, "name": name or s[:24]}
    if width:
        params["width"] = width
    cmd("text.create", **params)


def chip(x, y, label, palette, tone="muted", w=None):
    colour = {"muted": palette["muted"], "accent": palette["accent"],
              "good": palette["good"], "bad": palette["bad"], "warn": palette["warn"]}[tone]
    width = w or (len(label) * 7 + 18)
    rect(x, y, width, 20, palette["raised"], radius=10, stroke=palette["line"])
    text(x + 9, y + 5, label, 10, colour, width=width - 12)


def field(x, y, w, label, value, palette, hint=None):
    text(x, y, label, 10, palette["muted"])
    rect(x, y + 15, w, 30, palette["surface"], radius=6, stroke=palette["line"])
    text(x + 9, y + 24, value, 11, palette["text"], width=w - 18)
    if hint:
        text(x, y + 50, hint, 9, palette["muted"], width=w)


def window_chrome(p, title):
    rect(0, 0, 1440, 44, p["surface"], name="titlebar")
    rect(0, 43, 1440, 1, p["line"], name="titlebar rule")
    rect(16, 12, 20, 20, p["accent2"], radius=6, name="mark")
    text(44, 15, "Canonwright", 12, p["text"], bold=True)
    rect(120, 13, 1, 18, p["line"])
    text(134, 15, title, 11, p["muted"])
    # theme toggle
    rect(1336, 12, 44, 20, p["raised"], radius=10, stroke=p["line"])
    rect(1336, 12, 22, 20, p["accent2"], radius=10, name="theme knob")
    text(1392, 15, "Settings", 10, p["muted"])


# ---------------------------------------------------------------- 1. welcome
def welcome():
    p = LIGHT
    page("01 Welcome", p)
    window_chrome(p, "no project open")

    text(120, 170, "Write the book.", 44, p["text"], bold=True)
    text(120, 226, "Keep the canon.", 44, p["accent"], bold=True)
    text(120, 296, "Scenes, viewpoints, facts, rules and threads — held as state,", 13, p["muted"])
    text(120, 316, "so continuity is checked instead of remembered.", 13, p["muted"])

    rect(120, 380, 440, 190, p["surface"], radius=14, stroke=p["accent"])
    text(148, 406, "Start a new story", 17, p["text"], bold=True)
    text(148, 434, "A novel or a rulebook. Answer a short interview,", 11, p["muted"])
    text(148, 452, "or import a draft you already have.", 11, p["muted"])
    rect(148, 486, 150, 34, p["accent"], radius=8)
    text(172, 496, "New story", 12, "#FFFFFF", bold=True)
    text(316, 496, "or press ⌘N", 10, p["muted"])

    rect(592, 380, 440, 190, p["surface"], radius=14, stroke=p["line"])
    text(620, 406, "Open a project", 17, p["text"], bold=True)
    text(620, 434, "A folder, or a GitHub repository you already", 11, p["muted"])
    text(620, 452, "keep your manuscript in.", 11, p["muted"])
    rect(620, 486, 150, 34, p["raised"], radius=8, stroke=p["line"])
    text(644, 496, "Open folder", 12, p["text"], bold=True)

    text(120, 610, "RECENT", 9, p["muted"])
    rect(120, 630, 912, 1, p["line"])
    text(120, 650, "No projects yet. Everything you open appears here, most recent first.", 11, p["muted"])

    rect(120, 706, 912, 1, p["line"])
    text(120, 724, "Light and dark follow the system until you choose one.", 10, p["muted"])
    rect(1148, 716, 44, 20, p["raised"], radius=10, stroke=p["line"])
    rect(1148, 716, 22, 20, p["accent2"], radius=10)
    text(1204, 720, "Theme", 10, p["muted"])


# ------------------------------------------------------- 2. what are you writing
def step_one():
    p = LIGHT
    page("02 New story — what", p)
    window_chrome(p, "New story")

    rect(0, 44, 260, 856, p["surface"], name="rail")
    rect(260, 44, 1, 856, p["line"])
    text(32, 92, "NEW STORY", 10, p["muted"])
    for i, label in enumerate(["What you are writing", "Where it starts", "The interview"]):
        y = 130 + i * 44
        if i == 0:
            rect(20, y - 8, 220, 34, p["raised"], radius=8)
            rect(20, y - 8, 3, 34, p["accent"], radius=2)
            text(44, y, f"{i+1}   {label}", 12, p["text"], bold=True)
        else:
            text(44, y, f"{i+1}   {label}", 12, p["muted"])

    text(320, 120, "What are you writing?", 26, p["text"], bold=True)
    text(320, 160, "This decides the questions you are asked, the checks that run, and the shape of", 12, p["muted"])
    text(320, 178, "the workspace. You can change it later, but the defaults differ.", 12, p["muted"])

    # novel card, selected
    rect(320, 226, 470, 300, p["surface"], radius=14, stroke=p["accent"])
    rect(320, 226, 470, 4, p["accent"], radius=2)
    text(348, 254, "A novel", 20, p["text"], bold=True)
    text(348, 286, "Scenes with a viewpoint, characters with knowledge,", 11, p["muted"])
    text(348, 304, "threads that get planted and paid off.", 11, p["muted"])
    for i, line in enumerate([
        "Continuity: value shifts, causal links, dead characters",
        "Knowledge leaks: who could know what, and when",
        "Voice: per-character constraints on the prose",
    ]):
        rect(348, 344 + i * 26, 5, 5, p["accent"], radius=3)
        text(362, 340 + i * 26, line, 11, p["text"])
    chip(348, 446, "default", p, "accent", w=70)

    # rulebook card
    rect(820, 226, 470, 300, p["surface"], radius=14, stroke=p["line"])
    text(848, 254, "A rulebook", 20, p["text"], bold=True)
    text(848, 286, "Chapters, procedures, classes, items and monsters,", 11, p["muted"])
    text(848, 304, "with cross-references that must resolve.", 11, p["muted"])
    for i, line in enumerate([
        "Referential integrity: every \"see X\" points at X",
        "Completeness: a class with no numbers is flagged",
        "Balance: the cap you declare is checked against",
    ]):
        rect(848, 344 + i * 26, 5, 5, p["muted"], radius=3)
        text(862, 340 + i * 26, line, 11, p["text"])

    rect(320, 566, 970, 1, p["line"])
    text(320, 590, "Hybrid books — a setting with rules, a campaign with scenes — are supported:", 11, p["muted"])
    text(320, 608, "choose either and turn on the other's checks in the project settings.", 11, p["muted"])

    rect(1116, 806, 174, 38, p["accent"], radius=8)
    text(1160, 819, "Continue", 13, "#FFFFFF", bold=True)
    text(960, 819, "Cancel", 12, p["muted"])


# ---------------------------------------------------- 3. the interview (form)
def interview():
    p = LIGHT
    page("03 New story — interview", p)
    window_chrome(p, "New story · The interview")

    rect(0, 44, 260, 856, p["surface"], name="rail")
    rect(260, 44, 1, 856, p["line"])
    text(32, 92, "NEW STORY", 10, p["muted"])
    for i, label in enumerate(["What you are writing", "Where it starts", "The interview"]):
        y = 130 + i * 44
        if i == 2:
            rect(20, y - 8, 220, 34, p["raised"], radius=8)
            rect(20, y - 8, 3, 34, p["accent"], radius=2)
            text(44, y, f"{i+1}   {label}", 12, p["text"], bold=True)
        else:
            text(44, y, f"✓  {label}", 12, p["muted"])

    text(320, 104, "The interview", 26, p["text"], bold=True)
    text(320, 142, "Eleven questions. Only the title is required — a writer with half an idea must be", 12, p["muted"])
    text(320, 160, "able to start. Anything marked \"from draft\" was read out of the file you chose.", 12, p["muted"])

    text(320, 206, "IDENTITY", 9, p["muted"])
    field(320, 228, 330, "Title *", "The Salt Road", p)
    field(676, 228, 330, "Author", "Bruce Amoser", p)
    field(320, 300, 330, "Genre", "Fantasy", p)
    field(676, 300, 330, "Logline", "A caravan master learns who sold her the truce.", p)

    text(320, 380, "VOICE", 9, p["muted"])
    field(320, 402, 330, "Point of view", "Third limited", p)
    field(676, 402, 330, "Tense", "Past", p)
    chip(1016, 414, "from draft", p, "accent", w=76)

    text(320, 482, "CAST", 9, p["muted"])
    field(320, 504, 330, "Protagonist", "Mara Vale", p)
    field(676, 504, 330, "What do they want?", "Get the salt carts through the ford.", p)
    field(320, 576, 686, "What do they need that they would deny?", "Admit she left the valley out of fear.", p)

    text(320, 656, "STAKES", 9, p["muted"])
    field(320, 678, 686, "The question that keeps the reader turning", "Who burned the ford — and who profited?", p)

    # summary card
    rect(1050, 206, 320, 300, p["surface"], radius=12, stroke=p["line"])
    text(1074, 228, "Canonwright will create", 12, p["text"], bold=True)
    for i, (k, v) in enumerate([
        ("Kind", "Novel"),
        ("Scenes", "0 — you write the first"),
        ("Rules", "1 hard rule"),
        ("Thread", "The ford, planted in scene 1"),
        ("Workspace", "~/Canonwright/workspaces/the-salt-road"),
    ]):
        text(1074, 262 + i * 40, k, 9, p["muted"])
        text(1074, 276 + i * 40, v, 11, p["text"], width=270)

    rect(320, 806, 200, 38, p["surface"], radius=8, stroke=p["line"])
    text(356, 819, "Skip the rest", 12, p["muted"])
    rect(1030, 806, 260, 38, p["accent"], radius=8)
    text(1084, 819, "Create the project", 13, "#FFFFFF", bold=True)


# --------------------------------------------------------- 4. novel workspace
def novel_workspace():
    p = DARK
    page("04 Workspace — novel", p)
    window_chrome(p, "The Salt Road · Novel")

    # left rail: scenes
    rect(0, 44, 296, 856, p["surface"], name="scenes")
    rect(296, 44, 1, 856, p["line"])
    text(24, 68, "SCENES", 9, p["muted"])
    rect(214, 62, 60, 20, p["raised"], radius=6, stroke=p["line"])
    text(224, 66, "+ Scene", 10, p["text"])
    scenes = [
        ("1", "The ford", "canon", "good"),
        ("2", "The tally", "canon", "good"),
        ("3", "Ash on the water", "drafted", "accent"),
        ("4", "The seal", "planned", "muted"),
    ]
    for i, (n, title, status, tone) in enumerate(scenes):
        y = 100 + i * 54
        if i == 2:
            rect(12, y - 8, 272, 46, p["raised"], radius=8)
            rect(12, y - 8, 3, 46, p["accent"], radius=2)
        text(28, y, n, 10, p["muted"], mono=True)
        text(48, y - 2, title, 12, p["text"] if i == 2 else p["muted"], bold=(i == 2))
        text(48, y + 16, status, 9, p[tone])

    rect(0, 780, 296, 1, p["line"])
    text(24, 800, "CONTINUITY", 9, p["muted"])
    text(24, 822, "2 scenes need a recheck", 10, p["warn"])
    text(24, 842, "1 thread owed to the reader", 10, p["muted"])

    # centre: the scene
    text(336, 74, "Scene 3", 10, p["muted"])
    text(336, 96, "Ash on the water", 26, p["text"], bold=True)
    chip(336, 136, "drafted", p, "accent", w=66)
    chip(410, 136, "pov: Mara", p, "muted", w=76)
    chip(494, 136, "suspicion → certainty", p, "muted", w=160)

    for i in range(9):
        width = [860, 884, 812, 878, 640, 870, 852, 820, 470][i]
        rect(336, 186 + i * 30, width, 9, p["raised"], radius=4, o=0.9)

    rect(336, 470, 420, 120, p["surface"], radius=10, stroke=p["line"])
    text(356, 488, "BRIEF FOR THIS SCENE", 9, p["muted"])
    text(356, 510, "Mara knows the truce broke. She does not yet know Harek", 11, p["text"], width=390)
    text(356, 528, "burned the ford — she learns it at line 40.", 11, p["text"], width=390)
    text(356, 556, "Withheld: 1 later scene · 2 unrevealed facts", 9, p["accent"])

    # right: continuity
    rect(1136, 44, 304, 856, p["surface"], name="continuity")
    rect(1136, 44, 1, 856, p["line"])
    text(1160, 68, "CONTINUITY", 9, p["muted"])
    rect(1160, 88, 256, 78, p["raised"], radius=8, stroke=p["line"])
    text(1176, 100, "[CAUSALITY_BREAK]", 9, p["bad"], mono=True)
    text(1176, 118, "Scene 4 has no link to an earlier scene.", 10, p["text"], width=224)
    text(1176, 146, "advisory · dismiss or fix", 9, p["muted"])

    rect(1160, 176, 256, 78, p["raised"], radius=8, stroke=p["line"])
    text(1176, 188, "[DANGLING_SETUP]", 9, p["warn"], mono=True)
    text(1176, 206, "The forged seal was planted 6 scenes ago.", 10, p["text"], width=224)
    text(1176, 234, "confirmed against the timeline", 9, p["muted"])

    text(1160, 284, "AUDIT", 9, p["muted"])
    for i, (label, value) in enumerate([
        ("Knowledge leaks", "0"), ("Voice drift", "1"), ("Dirty scenes", "2"),
    ]):
        text(1160, 306 + i * 22, label, 10, p["muted"])
        text(1382, 306 + i * 22, value, 10, p["text"])

    rect(1160, 380, 256, 1, p["line"])
    text(1160, 396, "SOURCES", 9, p["muted"])
    text(1160, 416, "story.json · 4 scenes", 10, p["muted"], mono=True)
    text(1160, 434, "main · 2 ahead of origin", 10, p["muted"], mono=True)


# ------------------------------------------------------- 5. rulebook workspace
def rulebook_workspace():
    p = LIGHT
    page("05 Workspace — rulebook", p)
    window_chrome(p, "Deeds of Legend · Rulebook")

    rect(0, 44, 296, 856, p["surface"], name="chapters")
    rect(296, 44, 1, 856, p["line"])
    text(24, 68, "BOOK", 9, p["muted"])
    for i, (label, depth) in enumerate([
        ("Front matter", 0), ("1 · The core mechanic", 0), ("2 · Characters", 0),
        ("    Classes", 1), ("    Species", 1), ("3 · Adversaries", 0),
        ("4 · Equipment", 0), ("Appendix", 0),
    ]):
        y = 96 + i * 28
        if i == 3:
            rect(12, y - 6, 272, 26, p["raised"], radius=6)
        text(28, y, label, 11, p["text"] if i == 3 else p["muted"], bold=(i == 3))
    text(24, 380, "ENTITIES", 9, p["muted"])
    for i, (label, tone) in enumerate([
        ("Fighter (class)", "text"), ("Duelist (class)", "text"),
        ("Marsh-wight (monster)", "text"), ("Grapple (procedure)", "warn"),
    ]):
        text(28, 402 + i * 24, label, 10, p["text"] if tone == "text" else p["warn"])

    # centre: entity editor
    text(336, 74, "CLASS · 24 of 118", 9, p["muted"])
    text(336, 96, "Fighter", 26, p["text"], bold=True)
    text(336, 134, "A trained combatant who holds the line.", 12, p["muted"])

    text(336, 180, "FIELDS", 9, p["muted"])
    rect(336, 200, 760, 1, p["line"])
    fields = [
        ("Traits", "Might, Edge"),
        ("Hit points", "8 + Might at 1st level"),
        ("Starting gear", "One martial weapon, a shield"),
        ("Advancement", "Gain a talent every odd level"),
    ]
    for i, (k, v) in enumerate(fields):
        y = 214 + i * 58
        text(336, y, k, 10, p["muted"])
        rect(336, y + 16, 760, 30, p["surface"], radius=6, stroke=p["line"])
        text(346, y + 25, v, 11, p["text"])

    text(336, 462, "CROSS-REFERENCES", 9, p["muted"])
    for i, (target, state, tone) in enumerate([
        ("core-resolution", "resolves", "good"),
        ("dice", "resolves", "good"),
        ("grapple", "dangling — nothing defines it", "bad"),
    ]):
        y = 486 + i * 30
        text(336, y, target, 11, p["text"], mono=True)
        text(520, y, state, 10, p[tone])

    # right: checks
    rect(1136, 44, 304, 856, p["surface"], name="checks")
    rect(1136, 44, 1, 856, p["line"])
    text(1160, 68, "CHECKS", 9, p["muted"])
    rect(1160, 88, 256, 92, p["raised"], radius=8, stroke=p["bad"])
    text(1176, 100, "[BROKEN_REFERENCE]", 9, p["bad"], mono=True)
    text(1176, 118, "`grapple` is referenced but not defined.", 10, p["text"], width=224)
    text(1176, 146, "error — the book cannot ship with this", 9, p["muted"])

    rect(1160, 190, 256, 78, p["raised"], radius=8, stroke=p["line"])
    text(1176, 202, "[INCOMPLETE_ENTITY]", 9, p["warn"], mono=True)
    text(1176, 220, "Duelist has no fields and no summary.", 10, p["text"], width=224)
    text(1176, 248, "advisory", 9, p["muted"])

    text(1160, 296, "BALANCE", 9, p["muted"])
    text(1160, 318, "Trait cap: 5 at creation", 10, p["text"])
    text(1160, 338, "Highest in book: 5", 10, p["good"])
    text(1160, 358, "No chapter exceeds the cap.", 9, p["muted"])


# ------------------------------------------------------------ 6. source control
def source_control():
    p = DARK
    page("06 Source control", p)
    window_chrome(p, "The Salt Road · Source")

    rect(0, 44, 1440, 40, p["surface"], name="tabs")
    rect(0, 83, 1440, 1, p["line"])
    for i, label in enumerate(["Manuscript", "Canon", "Source"]):
        x = 24 + i * 118
        if i == 2:
            rect(x - 10, 54, 96, 28, p["raised"], radius=6)
            text(x, 60, label, 11, p["text"], bold=True)
        else:
            text(x, 60, label, 11, p["muted"])

    rect(24, 108, 900, 760, p["surface"], radius=12, stroke=p["line"])
    text(48, 130, "main", 14, p["text"], bold=True)
    text(96, 133, "↑2  ↓0   origin/main", 10, p["muted"])

    text(48, 172, "CHANGES · 3", 9, p["muted"])
    for i, (path, badge, tone) in enumerate([
        ("manuscript/scene-03.md", "M", "warn"),
        ("story.json", "M", "warn"),
        ("manuscript/scene-04.md", "U", "good"),
    ]):
        y = 196 + i * 30
        text(48, y, badge, 9, p[tone], mono=True)
        text(70, y - 1, path, 11, p["text"], mono=True)
        rect(846, y - 4, 54, 20, p["raised"], radius=6, stroke=p["line"])
        text(858, y, "stage", 9, p["muted"])

    text(48, 312, "STAGED · 1", 9, p["muted"])
    text(48, 336, "M", 9, p["good"], mono=True)
    text(70, 335, "story.json", 11, p["text"], mono=True)

    rect(48, 380, 852, 70, p["surface"], radius=8, stroke=p["line"])
    text(60, 394, "Scene 3: Mara works out who burned the ford.", 11, p["muted"])
    rect(48, 462, 120, 34, p["accent"], radius=8)
    text(76, 472, "Commit", 12, "#FFFFFF", bold=True)
    text(184, 472, "1 staged", 10, p["muted"])

    text(48, 528, "BRANCHES", 9, p["muted"])
    for i, (name, current) in enumerate([("main", True), ("draft/act-two", False)]):
        y = 552 + i * 26
        text(48, y, "●" if current else "○", 10, p["accent"] if current else p["muted"])
        text(70, y, name, 11, p["text"] if current else p["muted"], mono=True)

    text(48, 632, "TAG A DRAFT", 9, p["muted"])
    rect(48, 654, 300, 30, p["raised"], radius=6, stroke=p["line"])
    text(58, 663, "draft-2", 11, p["muted"])
    rect(360, 654, 120, 30, p["raised"], radius=6, stroke=p["line"])
    text(376, 663, "Tag & push", 10, p["text"])

    text(48, 724, "RECENT", 9, p["muted"])
    for i, (sha, subject) in enumerate([
        ("a3f21c9", "Scene 3: Mara works out who burned the ford"),
        ("7be04d1", "Scene 2: the tally-house"),
        ("12c9aa4", "Import the draft and set the cast"),
    ]):
        y = 748 + i * 24
        text(48, y, sha, 10, p["muted"], mono=True)
        text(120, y, subject, 11, p["text"])

    # right: what git means here
    rect(948, 108, 468, 760, p["surface"], radius=12, stroke=p["line"])
    text(972, 130, "This is the manuscript's home", 14, p["text"], bold=True)
    for i, line in enumerate([
        "Every commit is a draft you can return to.",
        "Every tag is a version you can hand to a reader.",
        "The assistant can stage, commit, push and tag;",
        "it asks before anything leaves the machine.",
    ]):
        text(972, 160 + i * 20, line, 11, p["muted"], width=420)

    text(972, 262, "REMOTE", 9, p["muted"])
    text(972, 284, "github.com/bruceamoser/the-salt-road", 11, p["text"], mono=True, width=420)

    text(972, 330, "FETCH / PULL / PUSH", 9, p["muted"])
    for i, label in enumerate(["Fetch", "Pull", "Push"]):
        rect(972 + i * 104, 352, 96, 30, p["raised"], radius=6, stroke=p["line"])
        text(996 + i * 104, 361, label, 10, p["text"])

    text(972, 424, "NEEDS A HUMAN", 9, p["muted"])
    rect(972, 446, 420, 60, p["raised"], radius=8, stroke=p["warn"])
    text(988, 458, "Merge conflict in manuscript/scene-03.md", 11, p["text"], width=380)
    text(988, 478, "Resolve in the editor, then continue the merge.", 10, p["muted"])


welcome()
step_one()
interview()
novel_workspace()
rulebook_workspace()
source_control()

cmd("page.delete", id=1)

out = Path(__file__).with_name("screens.json")
out.write_text(json.dumps(commands, indent=1))
print(f"{len(commands)} commands → {out}")
