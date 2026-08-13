---
name: bookwriter
description: Write a full book about a topic in Typst and compile it to PDF, keeping the source. Books get a title page, TOC, chapters with worked examples and figures, real cited references, and in-depth advanced sections near the end. Use when the user runs /bookwriter or asks to write a book, ebook, guide, or manual about some topic.
---

# Bookwriter

Write a complete, well-structured book on the requested topic. The deliverable
is **both** the compiled PDF and the Typst source tree — the source stays so
the book can be revised and recompiled later.

## Inputs

The topic comes from the invocation args. Infer the rest, don't interrogate:

- **Audience & depth**: infer from how the user phrased it ("intro to X" vs
  "advanced X internals"). Default: motivated reader new to the topic, taken
  to genuinely advanced material by the end.
- **Length**: default 40–70 pages. Respect explicit asks ("short book",
  "comprehensive reference").
- **Location**: `books/<slug>/` under the current directory unless the user
  names a place. `<slug>` is a short kebab-case name for the topic.

Only ask a question if the topic itself is ambiguous.

## Setup

Typst is usually not on PATH. Use nix:

```sh
nix run nixpkgs#typst -- compile main.typ <slug>.pdf
```

(or `nix shell nixpkgs#typst -c typst watch ...` for repeated compiles).

## Source layout

```
books/<slug>/
├── main.typ          # from templates/main.typ — settings, title page, TOC, includes
├── lib.typ           # from templates/lib.typ — shared helpers (callouts, ...)
├── chapters/
│   ├── 01-introduction.typ
│   ├── 02-....typ
│   └── ...
├── images/           # generated SVGs, if any
├── references.yml    # Hayagriva bibliography
└── <slug>.pdf        # compiled output
```

Copy `templates/main.typ`, `templates/lib.typ`, and `templates/references.yml`
from this skill's directory as starting points, then adapt
title/author/includes.

## Process

1. **Outline first.** Design the whole book before writing prose: chapter
   list with 2–4 section headings each.

   **Stay on topic.** The clear majority of chapters must be about the
   requested topic itself. A couple of chapters may cover related material or
   prerequisites — the "base" a reader needs before the main content — but
   never let background, adjacent topics, or general context grow to half the
   book or more. If the outline drifts that way, cut the background down and
   deepen the core chapters instead.

   Required shape:
   - Front: introduction — why the topic matters, who the book is for, roadmap.
   - Middle: fundamentals building progressively, each chapter leaning on the
     previous.
   - **Near the end: 2–3 in-depth advanced chapters** — internals, edge cases,
     performance, theory, open problems; the material an expert would still
     find interesting.
   - Back: a "further reading" section, optionally a glossary or appendices,
     and the bibliography.
2. **Gather references before writing.** If web access is available, verify
   sources; either way, cite **only works you are confident actually exist**
   (canonical books, seminal papers, official documentation). Never invent an
   author, title, or year. A short honest bibliography beats a long fabricated
   one. Populate `references.yml` and cite inline with `@key`.
3. **Write chapter by chapter**, one file per chapter, and **compile after
   every chapter** — Typst errors are much easier to localize per-file than
   after 60 pages. Fix warnings too (missing citations, unknown fonts).
4. **Final pass**: recompile, skim the PDF structure (`#outline` matches the
   promised roadmap, page count is in range), then deliver.

For long books, chapters may be drafted in parallel (e.g. one subagent per
chapter, given the outline and the reference keys) — but the outline, the
bibliography, and the final consistency pass must be done centrally so
terminology and notation stay uniform.

## Content requirements

Every chapter needs more than prose:

- **Worked examples** — code blocks for technical topics, concrete scenarios
  or case studies otherwise. At least one substantial example per chapter.
- **Figures** — diagrams, tables, plots. Aim for a figure every few pages.
  Use `#figure(..., caption: [...])` so they're numbered and referenceable
  with `@label`.
- **Citations** — anchor factual claims to the bibliography where a source
  exists.
- **Callouts** — use the template's `note`/`warning` boxes for asides, common
  pitfalls, and historical notes.
- Optionally end chapters with exercises or discussion questions when the
  topic suits it.

### Figures and images

Everything must compile offline-ish and be reproducible from source:

- Prefer **Typst-native** figures: `table`, `grid`, and shape primitives
  (`rect`, `circle`, `line`, `polygon`) cover most diagrams.
- For real diagrams (graphs, flowcharts, plots), use the **CeTZ** package
  (`#import "@preview/cetz:0.4.2"`). Note: `@preview` packages download on
  first use, so this needs network once; if that fails, fall back to native
  shapes or pre-generated SVGs.
- For anything complex, generate an **SVG** into `images/` (write a small
  script if needed) and embed with `#image("images/foo.svg")`.
- Never hotlink remote images, and never describe an image that isn't
  actually there.

## Typst gotchas

- Headings: `= Chapter`, `== Section`. The template turns every level-1
  heading into a chapter that starts on a fresh page.
- `#include` does **not** share the including file's scope — every chapter
  file must start with `#import "/lib.typ": *` to get the callout helpers.
  Put any new shared helper in `lib.typ`, not `main.typ`.
- Math: inline `$x^2$`, display math needs spaces inside: `$ x^2 $`.
- `#`, `@`, `$`, `_`, `*` are markup in prose — escape with `\` or wrap in
  `` `raw` `` when writing about them literally.
- Code blocks: fenced ` ```lang ` works and gets highlighting.
- Cross-reference figures/headings with `<label>` + `@label`.
- Citation keys are bare `@key` in prose; a key that's not in
  `references.yml` is a compile warning — don't ship with warnings.

## Delivery

Report the path to the PDF and the source directory, the final page count,
and the chapter list. Don't paste the book's content into the conversation.
