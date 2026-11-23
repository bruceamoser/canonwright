# Canonwright — logo brief

## What the product is

Canonwright is a writing harness for long-form fiction. Its whole argument is
that a story should be modelled as a **temporal state graph** — characters,
factions, world rules, facts, and plot threads, with every scene moving that
state — so continuity can be checked the way a compiler checks code. The name is
*canon* (the established truth of a story) + *wright* (a maker, as in
playwright, shipwright).

The audience is novelists, not engineers. The mark must feel like a **craftsman's
stamp**, not a SaaS logo.

## The mark: "the plumb nib"

A pen nib whose slit extends past the tip into a **plumb line** — a single
straight vertical rule that continues below the nib.

Two ideas in one silhouette:

- the nib: writing, the hand, the draft
- the plumb line: canon, the standard, the thing that must stay true

It survives at small sizes because it is one tapered shape plus one line, and it
is not a quill, an inkwell, or an open book — the three most exhausted marks in
this category.

### Generation prompt (primary)

> A flat vector logo mark for a fiction-writing tool called Canonwright. The mark
> is a pen nib seen head-on, rendered as a simple tapered wedge with a clean
> silhouette, whose central slit continues downward past the tip as a single
> straight vertical rule, as if a plumb line hangs from the nib. Geometric,
> symmetrical, hand-drawn-precision feel, thick and thin line weight variation
> like a woodcut. Two colours only: deep ink blue-black (#141B26) for the form
> and one warm accent, oxidised copper (#B4622D), for the rule. Solid flat
> fills, no gradients, no shading, no bevels, no drop shadows. Centred on a
> square canvas with generous margins. Isolated mark, no text, no letters, no
> wordmark, no frame, no background. Transparent background. Crisp edges,
> high contrast, legible when reduced to a 32-pixel favicon.

> Negative prompt: text, letters, typography, watermark, gradient, glow, 3D,
> bevel, shadow, texture, paper, quill, feather, inkwell, book, open book,
> scroll, compass, camera aperture, circuit board, robot, brain, clip art,
> stock vector, multiple marks, grid, mockup, photograph.

### Alternate A — "the seal"

> A flat vector logo mark: a circular wax-seal impression, simplified to a
> perfect circle with a single nib-shaped notch cut out of its lower edge, so the
> seal reads as both a stamp and a pen tip. Two colours: deep ink blue-black
> (#141B26) and a single warm accent, oxidised copper (#B4622D). Solid flat
> fills, no gradients, no texture, no 3D. Centred, generous margins, transparent
> background, no text, no letters, no frame. Must read clearly at 32 pixels.

### Alternate B — "the single thread"

> A flat vector logo mark built from one continuous line that enters from the
> left, crosses over itself exactly once in the centre, and exits right — a
> thread being kept track of. The crossing forms a small vertical stem. Single
> stroke weight with slight taper, deep ink blue-black (#141B26) with one warm
> copper (#B4622D) segment where the line crosses. Solid flat colour, no
> gradients, no shadow, no texture. Centred, transparent background, no text,
> no letters.

## Technical requirements (all variants)

| Requirement | Value |
| --- | --- |
| Master size | **2048 × 2048 px**, exported at 1024 × 1024 px |
| Format | PNG with **transparency** (RGBA). Also SVG if the tool exports vector |
| Shape | Perfectly square canvas, mark centred, **10 % safe margin** on all sides |
| Colours | **Two** maximum. Must read in pure black on white *and* white on black |
| Legibility | Must be recognisable at **32 × 32 px** — squint test, no fine detail |
| Background | None. Never bake in a background plate or rounded-rectangle tile |
| Text | **None.** The wordmark is a separate future task |

Deliver these three files:

1. `icon-master.png` — 2048 px, transparent, full colour
2. `icon-mono.png` — 2048 px, transparent, single colour (black), for tray and
   one-colour print
3. `icon-master.svg` — vector, if available

## Why these constraints

- **10 % safe margin**: macOS masks app icons into a rounded square and Windows
  tiles them. Art that touches the edge gets clipped.
- **Two colours**: the icon renders in a 16 px tray, a 32 px taskbar, a 512 px
  about box, and one day a printed spine. Gradients die at the first and third.
- **No baked background**: the app appears on light and dark desktops, and a
  pre-rounded tile looks wrong in Linux docks that add their own mask.
- **No text**: at 32 px, letters become grey mush, and the wordmark changes with
  every rebrand.

## What happens with the files

Once `icon-master.png` lands, the whole Tauri icon set is generated from it:

```sh
npx @tauri-apps/cli icon icon-master.png
# writes 32x32, 64x64, 128x128, 128x128@2x, 256x256, 512x512,
# icon.png, icon.icns (macOS), icon.ico (Windows) into apps/desktop/src-tauri/icons/
```

The installer and updater already read those paths, so the mark travels into the
`.deb`, `.AppImage`, `.dmg`, `.msi` and `.exe` without further changes.

## Acceptance checklist

- [ ] Reads at 32 px without a legend
- [ ] Works on white, black, and mid-grey
- [ ] No gradient, no shadow, no texture
- [ ] Transparent background, square, centred, 10 % margin
- [ ] Two colours, and a one-colour version exists
- [ ] Contains no letterforms
- [ ] Does not resemble a quill, an inkwell, an open book, or a camera aperture
- [ ] Survives being cropped to a circle (macOS) and a square (Windows)
