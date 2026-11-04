---
layout: ../../layouts/Guide.astro
title: Sync (beta)
description: How Noughtpad's Google Drive sync keeps one notebook the same on several computers, how to turn it on, what happens when the same note changes on two computers, and its current limits.
---

> **Sync is in beta.** It works between Macs today, used daily by a small group. During the beta, Google only lets invited people sign in, so you'll need an invitation: ask through the [contact form](/contact/). Sync is planned as a paid add-on later; there's no price yet.

Sync keeps the same notebook on two or more computers, through your own Google Drive.

## You may not need it

Your notes are ordinary files in a folder. If you already use Dropbox, Syncthing, iCloud Drive or git, you can keep the notes folder there and it will sync like any other folder. Noughtpad notices changes that arrive from outside and updates straight away.

## What Noughtpad sync does

- Copies your notes, images and other files, your chats, your "not related" choices and your trash to a folder in your Google Drive: **My Drive / Noughtpad / (your notes folder's name)**.
- Brings changes made on your other computers into this one.
- Noughtpad can only see the files it creates in your Drive. It can't read anything else there.

It doesn't sync the search index (each computer builds its own), your settings, your API keys, or downloaded models.

## Turn it on

1. Open **Settings**. Under **Sync**, click **Sign in with Google…**.
2. Your web browser opens. Choose your Google account and allow access. Then return to Noughtpad.
3. Turn on **Sync this notebook**.
4. Do the same on your other computer, with the same Google account.

The sign-in is kept in your computer's keychain.

## When it syncs

- About 5 seconds after you stop typing.
- Every minute.
- A few seconds after you switch back to the Noughtpad window.

So a change on one computer usually shows up on the other within a minute. If the note is open on the other computer, it updates in place, unless you're typing in it at that moment.

A small dot at the top right shows sync: it's highlighted while syncing and turns red if sync needs you (for example, to sign in again).

## When the same note changes on two computers

- **Different parts of a note:** both changes are kept.
- **Both added text at the same spot:** both additions are kept.
- **Labels:** labels added or removed on either computer are combined.
- **The same lines changed on both:** Noughtpad keeps one version in the note and saves the other as a separate copy next to it. Nothing is lost. Settings → Sync tells you when this happens, so you can merge the two by hand.
- **Deleted on one, edited on the other:** the edit wins and the note stays.

## Limits during the beta

- **Sign in again every 7 days.** During the beta, Google ends each sign-in after a week. Settings will say "Signed out"; click **Sign in with Google…** again. Nothing is lost while signed out; it catches up after.
- **Files over 50 MB** don't sync. Settings → Sync lists them.
- **Don't rename files inside Google Drive by hand.** Noughtpad would see a new file and a deleted one.
- **Windows and Linux** haven't been tested with sync as much as Macs.
- **iCloud sync and phone apps** are planned, not built.
