// Compile: nix run nixpkgs#typst -- compile main.typ book.pdf
#import "/lib.typ": *

#let title = [Your Book Title]
#let subtitle = [A one-line subtitle]
#let author = "Author Name"
#let date = datetime.today().display("[month repr:long] [year]")

#set document(title: title, author: author)
#set page(paper: "a4", margin: (x: 2.2cm, y: 2.5cm))
#set text(font: "New Computer Modern", size: 11pt)
#set par(justify: true)
#set heading(numbering: "1.1")

// Every level-1 heading is a chapter: fresh page, big title
#show heading.where(level: 1): it => {
  pagebreak(weak: true)
  v(1.5cm)
  text(size: 22pt, it)
  v(0.8cm)
}

#show raw.where(block: true): block.with(
  fill: luma(248),
  stroke: luma(220) + 0.5pt,
  inset: 10pt,
  radius: 4pt,
  width: 100%,
)

#show figure.caption: set text(size: 9pt, fill: luma(80))
#show link: set text(fill: rgb("#1a5276"))

// ---- Title page ----
#v(5cm)
#align(center)[
  #text(size: 30pt, weight: "bold", title)
  #v(0.5cm)
  #text(size: 14pt, style: "italic", subtitle)
  #v(2cm)
  #text(size: 12pt, author)
  #linebreak()
  #text(size: 10pt, fill: luma(100), date)
]
#pagebreak()

// ---- Table of contents ----
#outline(depth: 2)

#set page(numbering: "1")
#counter(page).update(1)

// ---- Chapters ----
#include "chapters/01-introduction.typ"
// #include "chapters/02-fundamentals.typ"
// ...

// ---- Bibliography ----
#bibliography("references.yml", style: "ieee")
