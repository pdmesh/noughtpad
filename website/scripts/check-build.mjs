// Post-build checks: every page has one h1, a title, a description and a canonical URL;
// every image has alt text; internal links point at pages or files that exist; and the
// copy avoids the words the style brief rules out.
import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join } from "node:path";

const dist = new URL("../dist/", import.meta.url).pathname;
const html = [];
const walk = (d) => readdirSync(d).forEach((f) => {
  const p = join(d, f);
  if (statSync(p).isDirectory()) walk(p);
  else if (p.endsWith(".html")) html.push(p);
});
walk(dist);

const banned = /\b(seamless(ly)?|effortless(ly)?|unlock|supercharge|elevate|revolutioni[sz]e|harness|delve|game-changer)\b/i;
const problems = [];
const titles = new Map();
for (const file of html) {
  const rel = file.slice(dist.length - 1);
  const s = readFileSync(file, "utf8");
  const h1 = (s.match(/<h1[\s>]/g) ?? []).length;
  if (h1 !== 1) problems.push(`${rel}: ${h1} h1 elements`);
  const title = s.match(/<title>([^<]*)<\/title>/)?.[1];
  if (!title) problems.push(`${rel}: no title`);
  else if (titles.has(title)) problems.push(`${rel}: same title as ${titles.get(title)}`);
  else titles.set(title, rel);
  if (!/<meta name="description" content="[^"]{50,}"/.test(s)) problems.push(`${rel}: missing or short description`);
  if (!/<link rel="canonical"/.test(s)) problems.push(`${rel}: no canonical`);
  for (const img of s.match(/<img\b[^>]*>/g) ?? []) if (!/\balt=/.test(img)) problems.push(`${rel}: image without alt: ${img.slice(0, 80)}`);
  const text = s.replace(/<script[\s\S]*?<\/script>|<style[\s\S]*?<\/style>/g, "").replace(/<[^>]+>/g, " ");
  const bad = text.match(banned);
  if (bad) problems.push(`${rel}: banned word "${bad[0]}"`);
  if (/—/.test(text)) problems.push(`${rel}: em dash in copy`);
  for (const [, href] of s.matchAll(/href="(\/[^"#?]*)/g)) {
    const target = href.endsWith("/") ? join(dist, href, "index.html") : join(dist, href);
    if (!existsSync(target)) problems.push(`${rel}: broken link ${href}`);
  }
}
if (problems.length) {
  console.error(problems.join("\n"));
  process.exit(1);
}
console.log(`check-build: ${html.length} pages OK`);
