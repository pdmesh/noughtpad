// Capture-speed budget (design risk #1): how long from opening the built UI
// until a keystroke lands in the editor. Runs the production bundle in a
// headless browser with the in-memory backend, so it measures the frontend
// (bundle size, startup work, editor mount) — the Rust side is measured by
// `cargo run --release -p noughtpad-core --example bench`.
//
//   pnpm build && pnpm perf            (BUDGET_MS overrides the budget)

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { chromium } from "playwright";

const DIST = new URL("../../dist/", import.meta.url).pathname;
const BUDGET_MS = Number(process.env.BUDGET_MS ?? 1500);
const RUNS = 5;
const TYPES = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".woff2": "font/woff2", ".svg": "image/svg+xml", ".png": "image/png" };

const server = createServer(async (req, res) => {
  const path = normalize(decodeURIComponent((req.url ?? "/").split("?")[0]));
  const file = join(DIST, path === "/" ? "index.html" : path);
  try {
    const body = await readFile(file.startsWith(DIST) ? file : join(DIST, "index.html"));
    res.writeHead(200, { "Content-Type": TYPES[extname(file)] ?? "application/octet-stream" });
    res.end(body);
  } catch {
    res.writeHead(404).end();
  }
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server.address().port}/`;

const browser = await chromium.launch();
async function measure(query) {
  const page = await browser.newPage();
  const t0 = Date.now();
  await page.goto(base + query);
  // Type as soon as the editor exists; done when the character is in the document.
  await page.waitForSelector(".cm-content[contenteditable=true]");
  await page.keyboard.type("x");
  await page.waitForFunction(() => document.querySelector(".cm-content")?.textContent?.includes("x"));
  const ms = Date.now() - t0;
  await page.close();
  return ms;
}

let failed = false;
for (const [name, query] of [
  ["small vault", ""],
  ["5,000 notes", "?mock=many"],
]) {
  await measure(query); // warm-up (browser start, disk cache)
  const times = [];
  for (let i = 0; i < RUNS; i++) times.push(await measure(query));
  times.sort((a, b) => a - b);
  const median = times[Math.floor(RUNS / 2)];
  const ok = median <= BUDGET_MS;
  failed ||= !ok;
  console.log(`${ok ? "ok  " : "SLOW"} ${name}: open → typing ${median} ms (median of ${RUNS}; budget ${BUDGET_MS} ms; runs ${times.join(", ")})`);
}
await browser.close();
server.close();
process.exit(failed ? 1 : 0);
