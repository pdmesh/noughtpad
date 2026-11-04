---
layout: ../../layouts/Guide.astro
title: Labels and sources
description: How to add labels to a Noughtpad note, record where an idea came from, and browse notes by label in Collections.
---

Labels and sources are optional. They stay hidden until you want them, so they never get in the way of writing.

## Labels

A label is a word or short phrase you attach to a note, like "Travel" or "Book ideas". A note can have as many labels as you like.

To add a label:

1. Move the pointer over the top of the note. A small strip appears with **Add label** and **Add source**.
2. Click **Add label** and start typing. Labels you've used before are suggested.
3. Press **Enter** to add it. Press **Backspace** in the empty box to remove the last one, or click the **×** on a label.

To see all notes with a label, open the sidebar and click the label under **Collections**. The number beside each label is how many notes have it.

In search, type `#travel` to find only notes with that label.

## Sources

A source is where an idea came from: a book, an article, a podcast, a talk. Add a title and, if you like, a web address.

1. Move the pointer over the top of the note.
2. Click **Add source**.
3. Type the title and, optionally, the link.

### Why sources matter

Noughtpad treats a note with a source as something you read or heard, and a note without one as your own thinking. That difference is used in a few places:

- **Journal** shows only your own notes, so it reads like a diary.
- **Ideas over time** can point out when something you read seems to have shaped a later note of yours.
- **Search** can narrow to `type:source` or `type:personal`.

You never have to set a note's type: adding a source is enough.

## Where they're stored

Labels and sources are written into a short header at the top of the note's file, like this:

```markdown
---
created: 2026-09-14T10:00:00+05:30
labels: [Product, Books]
source:
  title: The Mom Test
  url: https://example.com
---
```

Other Markdown apps, including Obsidian, can read this header too. [More about your files](/guide/where-files-live/)
