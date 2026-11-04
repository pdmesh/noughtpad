---
layout: ../../layouts/Guide.astro
title: Updating Noughtpad
description: How Noughtpad checks for and installs updates, how updates are verified, and what to expect after updating.
---

## How you'll know there's an update

Noughtpad checks for a new version quietly, shortly after it starts and then every few hours. When there is one, an **Update to …** button appears at the bottom of the sidebar.

Nothing is downloaded until you click it.

## Installing an update

1. Click **Update to …** at the bottom of the sidebar.
2. Noughtpad downloads the update, checks it, installs it and restarts. Your notes aren't affected.

You can also check by hand:

- **Settings → Updates → Check for updates**, or
- on a Mac, **Noughtpad → Check for Updates…** in the menu bar.

## How updates are checked

Updates are downloaded from GitHub. Every update is signed with Noughtpad's private key, and the app checks that signature before installing. An update that doesn't match is refused.

On a Mac, an update installed this way doesn't bring back the "can't be opened" warning you saw on first install.

## After an update on a Mac

The first start after an update can take several seconds longer than usual, with a "Opening your notes…" message. The second start is quick again. [Why this happens and how to avoid it](/guide/common-problems/#slow-first-launch)

## If the update button never appears

- Noughtpad must be in your **Applications** folder on a Mac, not running from the opened `.dmg`.
- **Very early versions (0.1.1 and before)** can't update themselves. Download the latest version from the [download page](/download/) and install it over the old one; your notes stay where they are.
- **Intel Macs** aren't supported, so they don't get updates.
