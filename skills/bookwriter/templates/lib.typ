// Shared helpers — import in every chapter with: #import "/lib.typ": *

// Callout boxes: #note[...] and #warning[...]
#let callout(title, color, body) = block(
  fill: color.lighten(88%),
  stroke: (left: color + 3pt),
  inset: (x: 12pt, y: 10pt),
  radius: (right: 4pt),
  width: 100%,
)[#text(weight: "bold", fill: color.darken(20%))[#title] \ #body]
#let note = callout.with("Note", rgb("#1a5276"))
#let warning = callout.with("Warning", rgb("#943126"))
