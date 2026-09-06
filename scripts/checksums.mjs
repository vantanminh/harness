#!/usr/bin/env node
/**
 * Generate a deterministic SHA-256 manifest for release binaries.
 *
 * Usage: node scripts/checksums.mjs --out SHA256SUMS [files...]
 * If no files are supplied, all target-scoped binaries in bin/ are used.
 */
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const targetPattern = /^harness-(?:x86_64|aarch64)-(?:unknown-linux-gnu|apple-darwin|pc-windows-msvc)(?:\.exe)?$/;

function fail(message) {
  console.error(`checksums: ${message}`);
  process.exit(1);
}

function assertNoSymlinkComponents(file, label, insideRepository = true) {
  const resolved = path.resolve(file);
  const relative = path.relative(root, resolved);
  if (insideRepository && (relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative))) {
    fail(`refusing ${label} outside the repository: ${relative}`);
  }
  const rootPath = path.parse(resolved).root;
  let current = rootPath;
  const components = resolved.slice(rootPath.length).split(path.sep).filter(Boolean);
  for (const part of components) {
    current = path.join(current, part);
    try {
      if (fs.lstatSync(current).isSymbolicLink()) {
        fail(`refusing ${label} through symlink: ${path.relative(root, current)}`);
      }
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
  }
}

const args = process.argv.slice(2);
let output = "SHA256SUMS";
const files = [];
for (let i = 0; i < args.length; i += 1) {
  const arg = args[i];
  if (arg === "--out") {
    output = args[++i] ?? fail("--out requires a path");
  } else if (arg === "--help" || arg === "-h") {
    console.log("Usage: node scripts/checksums.mjs [--out FILE] [binary ...]");
    process.exit(0);
  } else if (arg.startsWith("-")) {
    fail(`unknown option: ${arg}`);
  } else {
    files.push(arg);
  }
}

const selected = files.length > 0
  ? files.map((file) => path.resolve(root, file))
  : (assertNoSymlinkComponents(path.join(root, "bin"), "binary directory"),
    fs.readdirSync(path.join(root, "bin"), { withFileTypes: true }))
    .filter((entry) => entry.isFile() && targetPattern.test(entry.name))
    .map((entry) => path.join(root, "bin", entry.name));

if (selected.length === 0) fail("no release binaries supplied or staged in bin/");

for (const file of selected) {
  assertNoSymlinkComponents(file, "binary");
}

const rows = selected.map((file) => {
  if (!fs.existsSync(file) || !fs.lstatSync(file).isFile()) {
    fail(`binary does not exist: ${path.relative(root, file)}`);
  }
  const name = path.basename(file);
  if (!targetPattern.test(name)) {
    fail(`refusing non-target binary: ${name}`);
  }
  const digest = crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
  return { name, digest };
});

rows.sort((a, b) => a.name.localeCompare(b.name));
const duplicates = rows.map((row) => row.name).filter((name, index, all) => all.indexOf(name) !== index);
if (duplicates.length > 0) fail(`duplicate binary names: ${duplicates.join(", ")}`);

const destination = path.resolve(root, output);
assertNoSymlinkComponents(destination, "manifest destination");
if (selected.some((file) => path.resolve(file) === destination)) {
  fail("manifest destination must not overwrite a selected binary");
}
fs.writeFileSync(destination, `${rows.map(({ digest, name }) => `${digest}  ${name}`).join("\n")}\n`, {
  mode: 0o600,
});
console.log(`checksums: wrote ${rows.length} manifest row(s) to ${path.relative(root, destination)}`);
