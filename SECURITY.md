# Security

## Reporting a vulnerability

Please don't open a public issue for a security problem. Report it privately instead:

- **GitHub:** use [Report a vulnerability](https://github.com/pdmesh/noughtpad/security/advisories/new) on this repository's Security tab, or
- **The website:** write to us through [noughtpad.com/contact](https://noughtpad.com/contact/).

Include what you found, how to reproduce it, and which version you're on (shown in Settings). We'll acknowledge reports within a few days and keep you posted until it's fixed. We're happy to credit you in the release notes if you'd like.

## Supported versions

Noughtpad is in beta. Only the latest release gets security fixes; the app updates itself, so the fix reaches everyone through the normal update.

## What's in scope

- The desktop app (this repository) and its release builds.
- The update channel: releases are signed, and the app refuses an update whose signature doesn't match.
- noughtpad.com and its contact form (`website/`).

Out of scope: problems that need someone who already controls your computer or your notes folder, and issues in third-party services (Google Drive, model providers) that we can't fix.
