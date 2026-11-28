#set page(width: 6in, height: 9in, margin: (x: 50pt, top: 52pt, bottom: 50pt), numbering: "1")
#set text(font: ("Crimson Text", "Noto Serif"), size: 12.3pt, fill: rgb("#221f1b"))
#set par(justify: true, first-line-indent: 14pt, leading: 0.48em)
#show heading.where(level: 1): item => {
  pagebreak(weak: true)
  align(center, text(size: 22pt, item.body))
  v(12pt)
}
#align(center + horizon)[
#text(size: 30pt)[$title$]
$if(author)$
#v(18pt)
$for(author)$$author$$sep$, $endfor$
$endif$
]
#pagebreak()
$body$
