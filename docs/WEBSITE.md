# Website brief

For the sessions that build and host Noughtpad's website. Written 2026-11-04, when the app was at
v0.1.5. Read this, then `README.md` ("What works today") and `docs/DESIGN.md` for what the
product does and why. Use only what those say the app does today; don't promise anything else.

## What to build

1. **Marketing pages, written to be found in search:**
   - Home: what Noughtpad is in one line, who it's for, the main features, download.
   - Features: capture, related notes, search by meaning, ideas over time, journal and activity
     map, chat with your notes (your own model or provider), sync.
   - Download: links to the latest release on
     `https://github.com/pdmesh/noughtpad-releases/releases/latest` (public; installers only).
     Explain plainly how to open an unsigned app on a Mac (System Settings → Privacy & Security →
     Open Anyway), since the app isn't signed with an Apple certificate yet.
2. **Obsidian pages**, for people who search for Obsidian: an honest comparison page
   ("Noughtpad vs Obsidian") and, if it reads well, an "Obsidian alternative" page. Both apps keep
   notes as plain Markdown files in a folder, so a person can open the same folder in both. Be
   fair: say what Obsidian does better (plugins, community, maturity, mobile apps today). Check
   Obsidian's current prices and features on obsidian.md before stating them, and date the page.
3. **A user guide**: every feature, step by step, for someone who isn't technical. Start with
   install and first note; then labels and sources, search, related notes, ideas over time,
   journal, chat and models, sync, settings, keyboard shortcuts, where the files live, updating,
   and common problems (the unsigned-app prompt, the slow first launch after an update, signing
   in to Google again after 7 days while sync is in testing).

## How it should read

- Plain, simple English everywhere: short sentences, everyday words, no jargon without a
  one-line explanation.
- No AI-sounding writing: no "seamless", "effortless", "unlock", "supercharge", "elevate",
  "revolutionize", "harness", "delve", "game-changer", no em-dash flourishes, no lists of three
  adjectives. Say what the thing does.
- The product's own voice (see the app): calm, quiet, specific. "A blank page that quietly
  remembers."
- Claims must be true today. Where something is planned (iCloud, mobile apps, Windows/Linux
  sync testing, pricing), either leave it out or label it clearly as planned.

## SEO basics

Static pages, fast, no heavy scripts. A unique title and meta description per page, one `h1`,
real headings, `sitemap.xml`, `robots.txt`, canonical URLs, Open Graph and Twitter cards,
`SoftwareApplication` structured data on the home and download pages, and alt text on every
image. Screenshots of the real app (the sample notebook in `samples/vault` makes good,
non-private ones).

## Where it lives

- Code: a `website/` folder in this repo (`pdmesh/noughtpad`), on its own branch with a PR. The
  site generator is the session's choice (Astro is a good fit for static pages and Markdown
  guides); it must build to plain static files.
- Hosting: Cloudflare Pages, set up in a **separate session on the user's Mac**, where `wrangler`
  is installed and signed in. That session creates the Pages project, deploys `website/`'s build
  output, and connects the domain.

## Ask the user first

- The domain name.
- Pricing and how to describe sync (it's planned to be paid; nothing is decided).
- Whether to mention the open-source licence (FSL-1.1, Apache-2.0 after two years) and link the
  source (the repo is private today).
- Analytics (none, or a privacy-friendly one such as Cloudflare Web Analytics).
- A contact or support address.

## Answers (2026-11-04)

- **Domain:** noughtpad.com.
- **Pricing and sync:** the app is free. Sync is described as "in testing, by invitation", with a
  paid add-on planned and no prices.
- **Licence:** say "source available" only (no licence names on marketing pages). Link the source
  once `pdmesh/noughtpad` is public (`SOURCE_URL` in `website/src/config.ts`); the repo is to be
  audited first, and releases moved into it.
- **Analytics:** Cloudflare Web Analytics, turned on in the Pages project (no script in the code).
- **Contact:** hello@noughtpad.com (needs Email Routing on the domain).
- **Also asked for:** comparison pages for Notion, Logseq, Bear, Apple Notes and Evernote; privacy
  and terms pages kept out of search (noindex, not in the sitemap); a short demo video
  (HyperFrames).
- **Still open:** the legal name and governing law for the terms (`LEGAL_NAME`, `GOVERNING_LAW`).

Built in `website/`; see `website/README.md` for editing, regenerating media and deploying.
