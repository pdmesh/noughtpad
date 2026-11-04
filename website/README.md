# noughtpad.com

The Noughtpad website: marketing pages, comparisons, download page and user guide. Built with
[Astro](https://astro.build) to plain static files in `dist/`. No client-side JavaScript except
the hero video. The contact form is a plain HTML form handled by a small Worker (below). Brief: [`docs/WEBSITE.md`](../docs/WEBSITE.md).

```sh
cd website
npm ci            # needs Node 22.23.4 or later (Astro 7)
npm run dev       # http://localhost:4321
npm run build     # dist/, then scripts/check-build.mjs
npm run preview   # serve dist/
```

Copy rules: plain, confident, specific. No version numbers or hedges in marketing copy (the version shows on the download page only). Beta caveats, such as the one-time Mac confirmation, live in one guide section (`/guide/install-and-first-note/#first-open-mac`) and are linked from elsewhere. Comparison pages name the other app's main domain once, at the foot (`site` prop on `CompareLayout`).

`npm run build` fails if any page lacks one `h1`, a unique title, a description or a canonical
link, if an image has no alt text, if an internal link is broken, or if the copy uses a word the
brief rules out (seamless, unlock, supercharge…) or an em dash.

## What to edit

| What | Where |
|---|---|
| Version, release URL, source link, legal name | `src/config.ts` |
| Contact form and its result pages | `src/pages/contact/`, `functions/api/contact.ts`, `contact-worker/` |
| Home, features, download, compare, privacy, terms | `src/pages/*.astro` |
| User guide (one Markdown file per page) | `src/pages/guide/*.md`, order in `src/guide.ts` |
| Colours, type and layout | `src/styles/global.css`: paper, blue-black ink, graphite, one marker yellow used only as a highlight; Literata (self-hosted from `@fontsource-variable/literata`) for headings, the system face for text |
| Screenshots, social card, video | `public/img/`, `public/video/`; sources in `media/` |

**Each release:** bump `VERSION` in `src/config.ts`.

**When the main repo goes public:** set `SOURCE_URL` in `src/config.ts`. "Source available" then
appears in the footer, home page, terms and comparison tables. If releases move to the main repo,
change `RELEASES_URL` too.

**Before launch:** fill in `LEGAL_NAME` and `GOVERNING_LAW` in `src/config.ts` (the terms page
leaves out the governing-law clause while it's empty), and re-check the comparison pages against
each app's live pricing page; then update `COMPARISONS_CHECKED`.

**SEO notes:** sitemap at `/sitemap.xml` (built by `src/pages/sitemap.xml.ts`; privacy, terms
and 404 are left out). Those three pages carry `<meta name="robots" content="noindex">`. They are
deliberately *not* disallowed in `robots.txt`, because a crawler that can't fetch a page can't see
its `noindex`, and may list the bare URL anyway.

## Deploying to Cloudflare (from the Mac, with `wrangler` signed in)

Three pieces, all in the `noughtpad.com` zone:

| Piece | What | Deploy |
|---|---|---|
| Pages project `noughtpad` | The site (`dist/`) plus `functions/api/contact.ts`. Config in `wrangler.toml`. | `npm run build && npx wrangler pages deploy dist --project-name noughtpad --branch main` |
| Worker `noughtpad-contact` | Sends contact-form email through Email Routing. Reached from the Pages Function over the `CONTACT` service binding. | `cd contact-worker && npx wrangler deploy` |
| Worker `noughtpad-www` | 301 from `www.noughtpad.com` to the apex. Owns www as a Worker custom domain. | `cd www-redirect && npx wrangler deploy` |

Normally only the Pages deploy is needed.

**Contact form.** There is no email address on the site, to keep spam down. `/contact/` posts to
`/api/contact`; the Worker checks the fields, drops bots (a hidden honeypot field, link-stuffed
messages, foreign `Origin`), and sends one plain-text email from `website@noughtpad.com` with
`Reply-To` set to the sender. It then redirects to `/contact/sent/` or `/contact/problem/` (both
`noindex`, not in the sitemap). The destination is a Worker secret, not in the repo:
`cd contact-worker && npx wrangler secret put DESTINATION`. It must be a verified Email Routing
destination address. Email Routing must stay enabled on the zone.

**Dashboard-only settings** (the wrangler token can't change these):

1. **DNS:** the apex is a proxied `CNAME @ → noughtpad.pages.dev`, which the Pages custom domain
   `noughtpad.com` needs. www needs no record of ours; the Worker custom domain manages it.
2. **Web Analytics:** on for the Pages project (Workers & Pages → noughtpad → Metrics → Web
   Analytics). This injects Cloudflare's cookie-free beacon; nothing in the code changes. The
   privacy page already says so.

`public/_headers` sets security and cache headers; `404.html` is served for unknown paths.

## Regenerating screenshots, the social card and the video

Screenshots come from the app's browser-only UI (`pnpm dev` at the repo root, with the in-memory
mock backend), loaded with the 37-note sample notebook. Search by meaning is shown as on and sync
as signed in. The mock's chat answer is canned, not a real model's.

```sh
# from the repo root, with `pnpm dev` running on port 5173
node website/media/screenshots/seed.cjs       # sample notebook -> seed.json
node website/media/screenshots/final.cjs      # -> website/media/screenshots/out/*.png
node website/media/screenshots/og.cjs         # -> website/public/img/og.png
```

Convert each screenshot to the two WebP sizes the pages use:

```sh
cd website/media/screenshots/out
for f in *.png; do n=${f%.png}
  ffmpeg -y -i $f -c:v libwebp -quality 80 ../../../public/img/$n.webp
  ffmpeg -y -i $f -vf scale=1280:-1 -c:v libwebp -quality 82 ../../../public/img/$n-1280.webp
done
```

The demo video is a [HyperFrames](https://hyperframes.heygen.com) composition in `media/video/`
(HTML + GSAP; frames are the WebP screenshots). Copy fresh `write`, `related`, `thread`, `journal`
and `chat` WebPs into `media/video/assets/`, then:

```sh
cd website/media/video
npx hyperframes@0.9.11 check
npx hyperframes@0.9.11 render -o demo-raw.mp4
ffmpeg -y -i demo-raw.mp4 -vf "scale=1440:-2,fps=30" -c:v libx264 -preset slow -crf 27 \
  -pix_fmt yuv420p -movflags +faststart -an ../../public/video/demo.mp4
ffmpeg -y -ss 6 -i demo-raw.mp4 -frames:v 1 -vf scale=1440:-2 -c:v libwebp -quality 80 \
  ../../public/video/demo-poster.webp
```
