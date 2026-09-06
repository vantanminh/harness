#!/usr/bin/env node
/**
 * Stage native release artifacts downloaded from the CI matrix into bin/.
 *
 * Artifact uploads usually contain a generic `harness`/`harness.exe` inside a
 * target-named directory. The target name is therefore part of the path
 * contract, and this script refuses to guess when an artifact is ambiguous.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const TARGETS = [
  "x86_64-unknown-linux-gnu",
  "aarch64-unknown-linux-gnu",
  "x86_64-apple-darwin",
  "aarch64-apple-darwin",
  "x86_64-pc-windows-msvc",
  "aarch64-pc-windows-msvc",
];
const DEFAULT_TARGETS = TARGETS;

function fail(message) {
  console.error(`stage-native: ${message}`);
  process.exit(1);
}

function assertNoSymlinkComponents(file, label, insideRepository = false) {
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

function parseArgs() {
  const args = process.argv.slice(2);
  let from = "artifacts";
  let targets = process.env.HARNESS_NATIVE_TARGETS ?? DEFAULT_TARGETS.join(",");
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === "--from") {
      from = args[++i] ?? fail("--from requires a directory");
    } else if (arg === "--targets") {
      targets = args[++i] ?? fail("--targets requires a comma-separated list or all");
    } else if (arg === "--help" || arg === "-h") {
      console.log("Usage: node scripts/stage-native.mjs [--from DIR] [--targets TARGET[,TARGET...]|all]");
      process.exit(0);
    } else {
      fail(`unknown option: ${arg}`);
    }
  }
  const selected = targets === "all"
    ? TARGETS
    : targets.split(",").map((target) => target.trim()).filter(Boolean);
  if (selected.length === 0) fail("at least one native target is required");
  const unknown = selected.filter((target) => !TARGETS.includes(target));
  if (unknown.length > 0) fail(`unknown native target(s): ${unknown.join(", ")}`);
  return { from: path.resolve(root, from), targets: selected };
}

function walk(dir) {
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  const out = [];
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) out.push(...walk(full));
    else if (entry.isFile()) out.push(full);
  }
  return out;
}

function targetFilename(target) {
  return `harness-${target}${target.includes("windows") ? ".exe" : ""}`;
}

function sourceForTarget(files, target, used) {
  const expected = targetFilename(target);
  const exact = files.filter((file) => path.basename(file) === expected && !used.has(file));
  if (exact.length === 1) return exact[0];
  if (exact.length > 1) fail(`multiple ${expected} files found; keep one artifact per target`);

  const genericName = target.includes("windows") ? "harness.exe" : "harness";
  const scoped = files.filter(
    (file) =>
      path.basename(file) === genericName &&
      file.toLowerCase().includes(target.toLowerCase()) &&
      !used.has(file),
  );
  if (scoped.length === 1) return scoped[0];
  if (scoped.length > 1) fail(`multiple ${genericName} files found for ${target}`);
  fail(`no ${expected} artifact found under the target-scoped path`);
}

const { from, targets } = parseArgs();
assertNoSymlinkComponents(from, "artifact directory");
if (!fs.existsSync(from) || !fs.statSync(from).isDirectory()) {
  fail(`artifact directory does not exist: ${from}`);
}
const files = walk(from);
const binDir = path.join(root, "bin");
assertNoSymlinkComponents(binDir, "binary directory", true);
fs.mkdirSync(binDir, { recursive: true });
assertNoSymlinkComponents(binDir, "binary directory", true);
const used = new Set();
for (const target of targets) {
  const source = sourceForTarget(files, target, used);
  used.add(source);
  const destination = path.join(binDir, targetFilename(target));
  assertNoSymlinkComponents(source, "artifact");
  assertNoSymlinkComponents(destination, "binary destination", true);
  const stat = fs.lstatSync(source);
  if (stat.size === 0) fail(`artifact is empty: ${source}`);
  if (!stat.isFile()) fail(`artifact is not a regular file: ${source}`);
  if (fs.existsSync(destination) && fs.lstatSync(destination).isSymbolicLink()) {
    fail(`refusing to replace symlinked binary destination: ${destination}`);
  }
  fs.copyFileSync(source, destination);
  assertNoSymlinkComponents(destination, "binary destination", true);
  if (fs.lstatSync(destination).isSymbolicLink()) {
    fail(`refusing to use symlinked binary destination: ${destination}`);
  }
  if (!target.includes("windows")) fs.chmodSync(destination, 0o755);
  console.log(`staged ${target} ← ${path.relative(root, source)}`);
}

console.log(`stage-native: ${targets.length} target(s) staged in ${path.relative(root, binDir)}`);
