---
layout: ../../layouts/Guide.astro
title: Where your files live
description: Where Noughtpad saves your notes, how files and folders are named, what is in the hidden .noughtpad folder, and how to back up or move your notes.
---

Your notes are ordinary text files in the folder you chose when you first opened Noughtpad. By default that's **Documents/Noughtpad**. To see which folder you're using, open **Settings → Notes folder**, or look at the bottom of the sidebar.

## How notes are named

Each note is one Markdown (`.md`) file, filed by year and month:

```text
Noughtpad/
  2026/
    09/
      2026-09-12-2310-ownership-seems-to-matter.md
      2026-09-14-1000-customers-describe-what-they-want.md
    10/
      2026-10-02-0730-is-good-company-the-right.md
  attachments/
    pasted-image.png
  .noughtpad/
```

The file name is the date and time you started the note, plus a few words from its first line or title. It's set once and never changes, even if you edit the first line later.

## Inside a note

A note file starts with a short header (between two `---` lines), then your text:

```markdown
---
id: 01KJTAV4J0F5KZNWJ47TAN9ZT2
created: 2026-03-03T22:40:00+05:30
labels: [VC]
---

Ownership may matter more than I expected.
```

The `id` lets Noughtpad recognise a note even if you rename or move the file. You can open, edit, rename or move notes in any other app; Noughtpad notices straight away. It reads Markdown files it didn't create too, including ones with no header at all.

Noughtpad saves safely: a note is written to a temporary file first and then swapped in, so a crash can't leave half a note. It never rewrites your files behind your back.

## Images

Images you paste are saved in the `attachments` folder and linked from the note.

## The hidden .noughtpad folder

Noughtpad keeps its own data in a hidden folder called `.noughtpad` inside your notes folder:

| What | Can it be rebuilt? |
|---|---|
| The search index | Yes. Delete it and Noughtpad rebuilds it from your notes. |
| Your chats (`chats`) | No. Keep it if you want your chat history. |
| Your "not related" choices and what related notes learned (`user-state`) | No. |
| Deleted notes (`trash`) | No. This is where to look if you need a deleted note back. |

On a Mac, press **⌘⇧.** (Command, Shift, full stop) in Finder to show hidden folders.

Things that are **not** in the notes folder: your settings, your API keys (kept in your computer's keychain) and downloaded models (kept in Noughtpad's own app data folder).

## Backing up

Back up the whole notes folder, including `.noughtpad`, with whatever you already use: Time Machine, a cloud drive or git. If you use git, you can leave out `.noughtpad/index.sqlite`, since it can be rebuilt.

## Moving your notes

1. Quit Noughtpad.
2. Move the whole notes folder where you want it.
3. Open Noughtpad, go to **Settings → Notes folder → Switch…**, and choose the new place.

## Using the same folder in Obsidian

You can open your Noughtpad folder as a vault in Obsidian, or point Noughtpad at an existing vault. Both apps read and write plain Markdown, so edits in one show up in the other. Obsidian shows Noughtpad's labels as a property called `labels`. [Noughtpad vs Obsidian](/noughtpad-vs-obsidian/)
