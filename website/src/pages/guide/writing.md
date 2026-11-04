---
layout: ../../layouts/Guide.astro
title: Writing and formatting
description: How the Noughtpad editor works, what Markdown formatting it shows, and how to paste, copy, add images, diagrams and maths.
---

Noughtpad notes are written in Markdown, a simple way to format text with ordinary characters. You don't have to learn it: plain text works fine. But if you use it, Noughtpad shows the result straight away.

## What you see as you type

The Markdown symbols are hidden except on the line you're editing. Move the cursor away and the line looks finished.

| Type this | You get |
|---|---|
| `# Heading` | A large heading (`##` and `###` for smaller ones) |
| `**bold**` | **bold** |
| `*italic*` | *italic* |
| `- item` | A bullet list |
| `1. item` | A numbered list |
| `- [ ] task` | A checkbox you can click |
| `> quote` | A quote |
| `` `code` `` | Code, in a fixed-width font |
| `[text](https://example.com)` | A link |

## Titles are optional

You don't need a title. Noughtpad uses the first line of the note as its name in lists. If you'd like a separate title, move the pointer over the top of the note and add one.

## Paste and copy

- **Paste from the web, Google Docs or Word.** Headings, lists, links and tables come across as tidy Markdown.
- **Paste plain text only.** Press **⌘⇧V** (Ctrl+Shift+V).
- **Paste a link over selected text** to turn that text into a link.
- **Paste an image** (for example a screenshot) and it's saved in an `attachments` folder inside your notes folder and shown in the note.
- **Copy from Noughtpad** and paste into an email or document: you get formatted text. Paste into a Markdown app and you get Markdown.

## Diagrams and maths

To draw a diagram, write it in Mermaid inside a code block marked `mermaid`:

````markdown
```mermaid
graph LR
  Idea --> Note --> Thread
```
````

For maths, use LaTeX: `$E = mc^2$` inside a line, or `$$ … $$` on lines of their own. Diagrams and maths show rendered when the cursor is outside them.

## Deleting a note

Move the pointer over the top of the note and click the bin icon. The note goes to a trash folder inside your notebook (`.noughtpad/trash`), so you can get it back from there if you need to.

## Hiding and showing the sidebar

The sidebar and toolbar appear when the pointer nears the edge of the window and hide again when you type. Press **⌘\\** (Ctrl+\\) to keep them on screen, and again to hide them.
