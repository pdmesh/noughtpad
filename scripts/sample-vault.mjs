#!/usr/bin/env node
// Copy the sample notebook (samples/vault) to a scratch folder and, with
// --run, start `tauri dev` on it. Your configured notes folder is untouched.
//
//   pnpm sample                      copy to ~/Nought Sample (keeps your edits)
//   pnpm sample --reset              start again from the pristine samples
//   pnpm sample --to <folder>        copy somewhere else
//   pnpm dev:sample                  copy, then run the app on that folder
import { cpSync, existsSync, rmSync, writeFileSync, readdirSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const src = join(repo, "samples", "vault");
const toIdx = args.indexOf("--to");
const dest = resolve(toIdx >= 0 ? args[toIdx + 1] : join(homedir(), "Nought Sample"));
const MARKER = ".nought-sample";

if (args.includes("--reset") && existsSync(dest)) {
  // Only ever delete a folder this script created.
  if (!existsSync(join(dest, MARKER))) {
    console.error(`Refusing to reset ${dest}: it wasn't created by this script.`);
    process.exit(1);
  }
  rmSync(dest, { recursive: true, force: true });
}

const fresh = !existsSync(dest) || readdirSync(dest).length === 0;
// Copy without overwriting, so edits made while testing survive a re-run.
cpSync(src, dest, { recursive: true, force: false, errorOnExist: false });
writeFileSync(join(dest, MARKER), "Created by scripts/sample-vault.mjs. Safe to delete.\n");
console.log(`${fresh ? "Created" : "Updated"} sample notebook at ${dest}`);

if (args.includes("--run")) {
  const child = spawn("pnpm", ["tauri", "dev"], {
    cwd: repo,
    stdio: "inherit",
    shell: process.platform === "win32",
    env: { ...process.env, NOUGHT_VAULT: dest },
  });
  child.on("exit", (code) => process.exit(code ?? 0));
}
