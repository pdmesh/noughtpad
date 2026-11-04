# noughtpad.com

The Noughtpad website: marketing pages, comparisons, download page and user guide. Built with
[Astro](https://astro.build) to plain static files in `dist/`. No client-side JavaScript except
the hero video. Brief: [`docs/WEBSITE.md`](../docs/WEBSITE.md).

```sh
cd website
npm ci
npm run dev       # http://localhost:4321
npm run build     # dist/, then scripts/check-build.mjs
npm run preview   # serve dist/
```

`npm run build` fails if any page lacks one `h1`, a unique title, a description or a canonical
link, if an image has no alt text, if an internal link is broken, or if the copy uses a word the
brief rules out (seamless, unlock, supercharge…) or an em dash.

## What to edit

| What | Where |
|---|---|
| Version, release URL, source link, contact, legal name | `src/config.ts` |
| Home, features, download, compare, privacy, terms | `src/pages/*.astro` |
| User guide (one Markdown file per page) | `src/pages/guide/*.md`, order in `src/guide.ts` |
| Colours and layout | `src/styles/global.css` (the app's own tokens) |
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

## Deploying to Cloudflare Pages (from the Mac, with `wrangler` signed in)

```sh
cd website && npm ci && npm run build
npx wrangler pages project create noughtpad --production-branch main
npx wrangler pages deploy dist --project-name noughtpad --branch main
```

Then in the Cloudflare dashboard:

1. **Workers & Pages → noughtpad → Custom domains:** add `noughtpad.com`, and `www.noughtpad.com`
   with a redirect to the apex (Rules → Redirect Rules, or a Bulk Redirect).
2. **Web Analytics:** turn it on for the Pages project (Metrics → Web Analytics → Enable). This
   injects Cloudflare's cookie-free beacon; nothing in the code needs to change. The privacy page
   already says so.
3. **Email:** `hello@noughtpad.com` appears on every page. Set up Email Routing
   (domain → Email → Email Routing) to forward it to a real inbox.

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
