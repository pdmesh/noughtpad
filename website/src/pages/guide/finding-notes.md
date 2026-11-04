---
layout: ../../layouts/Guide.astro
title: All notes, collections and search
description: How to find notes in Noughtpad with All notes, Collections, the search box and search filters, and how to turn on search by meaning.
---

## All notes

**All notes** in the sidebar lists every note, newest first, grouped by day. Click a note to open it. The list loads more as you scroll, so it stays quick however many notes you have.

If you'd like a list of recent notes under the writing page, press **⌘K** and choose **Show note list under the editor**, or turn on **Settings → Layout → Show recent notes under the editor**.

## Collections

Under **Collections** in the sidebar you'll find every label you've used. Click one to see only those notes. [More about labels](/guide/labels-and-sources/)

## Search

Press **⌘K** (Ctrl+K on Windows and Linux), or click the magnifying glass at the top right. Type and results appear as you go, with the matching words highlighted. Press **Enter** to open the top result, or use the arrow keys to pick another.

You can narrow a search:

| Type | Finds |
|---|---|
| `ownership fund` | Notes with both words |
| `"fund outcome"` | That exact phrase |
| `own` | Words starting with "own" too: own, owner, ownership |
| `#travel` | Only notes labelled Travel |
| `type:source` | Only notes with a source |
| `type:personal` | Only your own notes |

These can be combined, for example `#books type:source habits`.

The same box also runs commands. Type "settings", "journal" or "related" to jump there.

## Search by meaning

Word search only finds the words you type. Search by meaning also finds notes that say the same thing differently: a search for "getting fit" can find a note about half-marathon training.

It's off until you turn it on:

1. Open **Settings**.
2. Under **Finding connections**, turn on **Search by meaning, related notes and ideas over time**.
3. Noughtpad downloads a small model (about 65 MB) once, then reads through your notes. This takes a few seconds for a few hundred notes.

The model runs on your computer. Your notes are not sent anywhere. Results found by meaning rather than by matching words are marked "similar meaning".

## If search seems out of date

Search normally catches up within a second of any change, including changes made in other apps. If it ever seems wrong, open **Settings** and click **Rebuild** next to **Rebuild the search index**. Your notes aren't touched.
