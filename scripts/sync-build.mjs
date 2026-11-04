#!/usr/bin/env node
// Run a command with Noughtpad Sync built in: the open stand-in crate
// (crates/noughtpad-sync-stub) is patched to the private crate
// pdmesh/noughtpad-sync, checked out next to this repo (or at
// $NOUGHTPAD_SYNC_DIR). Cargo.lock is put back afterwards, so the open lock
// file never mentions the private crate.
//
//   pnpm sync pnpm tauri dev
//   pnpm sync pnpm tauri build --features local-llm-metal --bundles app
//   pnpm sync cargo check -p noughtpad-app
//
// Sign-in needs NOUGHTPAD_GOOGLE_CLIENT_SECRET in the environment.
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const stub = join(root, "crates", "noughtpad-sync-stub");
const sync = resolve(process.env.NOUGHTPAD_SYNC_DIR ?? join(root, "..", "noughtpad-sync"));
const args = process.argv.slice(2);

if (args.length === 0) {
  console.error("usage: node scripts/sync-build.mjs <command> [args…]");
  process.exit(2);
}
if (!existsSync(join(sync, "Cargo.toml"))) {
  console.error(`Noughtpad Sync not found at ${sync}. Clone pdmesh/noughtpad-sync there, or set NOUGHTPAD_SYNC_DIR.`);
  process.exit(1);
}
if (!process.env.NOUGHTPAD_GOOGLE_CLIENT_SECRET) {
  console.warn("NOUGHTPAD_GOOGLE_CLIENT_SECRET is not set: this build can't sign in to Google Drive.");
}

const cargoDir = join(root, ".cargo");
const config = join(cargoDir, "config.toml");
if (existsSync(config) && !readFileSync(config, "utf8").includes("sync-build.mjs")) {
  console.error(`${config} exists and wasn't written by this script; not touching it.`);
  process.exit(1);
}
const toml = (s) => JSON.stringify(s); // TOML basic strings use the same escapes
mkdirSync(cargoDir, { recursive: true });
writeFileSync(
  config,
  `# Written by scripts/sync-build.mjs; removed when it finishes.\n` +
    `[patch.${toml(pathToFileURL(stub).href)}]\n` +
    `noughtpad-sync = { path = ${toml(sync)} }\n`,
);
const lock = join(root, "Cargo.lock");
const lockBackup = `${lock}.sync-build`;
copyFileSync(lock, lockBackup);

const restore = () => {
  rmSync(config, { force: true });
  if (existsSync(lockBackup)) {
    copyFileSync(lockBackup, lock);
    rmSync(lockBackup);
  }
};
for (const sig of ["SIGINT", "SIGTERM"]) process.on(sig, () => (restore(), process.exit(130)));

const result = spawnSync(args[0], args.slice(1), { stdio: "inherit", cwd: process.cwd(), shell: process.platform === "win32" });
restore();
process.exit(result.status ?? 1);
