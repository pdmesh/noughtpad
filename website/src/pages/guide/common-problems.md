---
layout: ../../layouts/Guide.astro
title: Common problems
description: Fixes for common Noughtpad problems, including the macOS "can't be opened" warning, a slow first start after an update, signing in to Google again for sync, and chat models that won't load.
---

<h2 id="unsigned-app">"Noughtpad can't be opened" on a Mac</h2>

Noughtpad isn't signed with an Apple developer certificate yet, so macOS blocks it the first time.

1. Click **Done** on the warning.
2. Open **System Settings → Privacy & Security**.
3. Scroll to **Security** and click **Open Anyway** next to the Noughtpad line.
4. Enter your password if asked and click **Open Anyway** again.

You only need to do this once. Updates installed from inside Noughtpad don't ask again.

<h2 id="smartscreen">"Windows protected your PC"</h2>

Click **More info**, then **Run anyway**. Windows shows this because the installer isn't signed yet.

<h2 id="slow-first-launch">The first start after an update is slow (Mac)</h2>

After installing an update, Noughtpad may show "Opening your notes…" for several seconds (about 6 seconds has been seen). The next start is quick again, under half a second.

**Why:** macOS protects the Documents, Desktop and Downloads folders, and iCloud Drive. Each time an unsigned app changes, macOS treats it as a new app and checks it again the first time it reads from one of those folders.

**What you can do:**

- Nothing. It only happens once per update.
- Or keep your notes outside those folders, for example in a folder called **Noughtpad** in your home folder. Quit Noughtpad, move the folder, then choose it in **Settings → Notes folder → Switch…**.

This should go away once Noughtpad is signed with an Apple certificate.

<h2 id="sync-signed-out">Sync says "Signed out"</h2>

While sync is in testing, Google ends each sign-in after 7 days. Open **Settings → Sync** and click **Sign in with Google…** again. Nothing is lost while you're signed out; sync catches up once you're back.

If Google says you don't have access, sync is still limited to invited testers. Write to [hello@noughtpad.com](mailto:hello@noughtpad.com).

<h2 id="conflict-copy">A note appeared twice after syncing</h2>

When the same lines of a note were changed on two computers before they synced, Noughtpad keeps both versions: one in the note and one as a separate copy next to it. Settings → Sync tells you when this happens. Open both, keep what you want, and delete the copy.

<h2 id="no-connections">Related notes, search by meaning or Ideas over time show nothing</h2>

- Check they're turned on: **Settings → Finding connections**.
- Ideas over time needs at least eight notes, and a thread needs three or more notes written on at least two different days.
- Related notes only shows notes it's fairly sure about. Short notes on unrelated topics are left out on purpose.

<h2 id="model-wont-load">The chat model won't load</h2>

Models on your computer need free graphics memory. Other apps that run models, such as Ollama or LM Studio, often keep one loaded. Quit them, or unload their model, and try again. On a computer with 8 GB of memory or less, try the smaller **Qwen3 1.7B** on the Models page.

<h2 id="stale-search">Search seems out of date</h2>

Open **Settings** and click **Rebuild** next to **Rebuild the search index**. Your notes aren't touched.

<h2 id="quick-capture">Quick capture (⌘⇧Space) doesn't work</h2>

Noughtpad needs to be running. On a Mac, closing the window keeps it running, but quitting (**⌘Q**) stops it. Turn on **Settings → Start-up → Open at login** to have it ready whenever you log in. Another app may also be using the same shortcut.

## Still stuck?

Write to [hello@noughtpad.com](mailto:hello@noughtpad.com). It helps to say which computer you use, your Noughtpad version (in **Settings → Updates**) and what you were doing when it happened.
