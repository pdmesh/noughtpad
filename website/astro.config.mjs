// @ts-check
import { defineConfig } from "astro/config";
import { SITE_URL } from "./src/config.ts";

// Plain static output: every page is an HTML file in dist/, ready for Cloudflare Pages.
export default defineConfig({
  site: SITE_URL,
  trailingSlash: "always",
  build: { format: "directory" },
  compressHTML: true,
});
