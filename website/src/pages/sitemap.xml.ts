// Builds /sitemap.xml from the pages in this folder. Legal pages, 404 and the contact-form result pages are left out
// on purpose: they carry `noindex` and shouldn't be offered to search engines.
import type { APIRoute } from "astro";
import { SITE_URL } from "../config";

const EXCLUDE = new Set(["/404/", "/privacy/", "/terms/", "/contact/sent/", "/contact/problem/"]);

export const GET: APIRoute = () => {
  const files = Object.keys(import.meta.glob("./**/*.{astro,md}"));
  const paths = files
    .map((f) => f.replace(/^\.\//, "/").replace(/\.(astro|md)$/, "").replace(/\/index$/, "") + "/")
    .map((p) => (p === "//" ? "/" : p))
    .filter((p) => !p.includes("[") && !EXCLUDE.has(p))
    .sort();
  const body =
    `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n` +
    paths.map((p) => `  <url><loc>${SITE_URL}${p}</loc><lastmod>2026-11-04</lastmod></url>`).join("\n") +
    `\n</urlset>\n`;
  return new Response(body, { headers: { "Content-Type": "application/xml" } });
};
