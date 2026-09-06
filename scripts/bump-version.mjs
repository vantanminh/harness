#!/usr/bin/env node
/**
 * Bump package version and keep release-critical files in sync.
 *
 * Usage:
 *   node scripts/bump-version.mjs              # default: patch
 *   node scripts/bump-version.mjs patch|minor|major
 *   node scripts/bump-version.mjs 1.2.3        # set exact version
 *
 * Updates:
 *   - package.json
 *   - package-lock.json (root + packages[""])
 *   - Cargo.toml           (package.version)
 *   - templates/AGENTS.md  <!-- harness-version: X.Y.Z -->
 *   - AGENTS.md            (same marker, when present)
 *
 * Prints the new version to stdout (last line is just the version for CI).
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const SEMVER_RE = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+([0-9A-Za-z.-]+))?$/;

function fail(message) {
  console.error(`bump-version: ${message}`);
  process.exit(1);
}

function assertNoSymlinkComponents(file, label) {
  const resolved = path.resolve(root, file);
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

function readText(rel, required = true) {
  const full = path.join(root, rel);
  assertNoSymlinkComponents(full, rel);
  if (!fs.existsSync(full)) {
    if (required) fail(`${rel} is missing`);
    return null;
  }
  const stat = fs.lstatSync(full);
  if (!stat.isFile() || stat.isSymbolicLink()) {
    fail(`${rel} must be a regular file`);
  }
  return fs.readFileSync(full, "utf8");
}

function writeText(rel, text) {
  const full = path.join(root, rel);
  assertNoSymlinkComponents(full, rel);
  if (fs.existsSync(full)) {
    const stat = fs.lstatSync(full);
    if (!stat.isFile() || stat.isSymbolicLink()) {
      fail(`${rel} must be a regular file`);
    }
  }
  fs.writeFileSync(full, text, "utf8");
}

function parseSemver(v) {
  const m = String(v).trim().match(SEMVER_RE);
  if (!m) fail(`invalid semver: ${v}`);
  return {
    major: Number(m[1]),
    minor: Number(m[2]),
    patch: Number(m[3]),
    prerelease: m[4] ?? null,
    build: m[5] ?? null,
  };
}

function formatSemver({ major, minor, patch, prerelease, build }) {
  let out = `${major}.${minor}.${patch}`;
  if (prerelease) out += `-${prerelease}`;
  if (build) out += `+${build}`;
  return out;
}

function bump(current, kind) {
  const p = parseSemver(current);
  if (kind === "major") {
    return formatSemver({ major: p.major + 1, minor: 0, patch: 0 });
  }
  if (kind === "minor") {
    return formatSemver({ major: p.major, minor: p.minor + 1, patch: 0 });
  }
  if (kind === "patch") {
    return formatSemver({ major: p.major, minor: p.minor, patch: p.patch + 1 });
  }
  // exact version
  parseSemver(kind); // validate
  return kind;
}

function readJson(rel) {
  return JSON.parse(readText(rel));
}

function writeJson(rel, data) {
  writeText(rel, `${JSON.stringify(data, null, 2)}\n`);
}

function replaceInFile(rel, replacer) {
  const before = readText(rel, false);
  if (before === null) return false;
  const after = replacer(before);
  if (after === before) return false;
  writeText(rel, after);
  return true;
}

const arg = (process.argv[2] ?? "patch").trim();
const kind = ["patch", "minor", "major"].includes(arg) ? arg : arg;

const pkg = readJson("package.json");
const oldVersion = pkg.version;
const newVersion = bump(oldVersion, kind);

if (newVersion === oldVersion) {
  fail(`version unchanged: ${oldVersion}`);
}

// package.json
pkg.version = newVersion;
writeJson("package.json", pkg);

// package-lock.json (root fields only — keep deps intact)
const lockText = readText("package-lock.json", false);
if (lockText !== null) {
  const lock = JSON.parse(lockText);
  lock.version = newVersion;
  if (lock.packages && lock.packages[""]) {
    lock.packages[""].version = newVersion;
    // keep lock name aligned with package.json when present
    if (pkg.name) {
      lock.name = pkg.name;
      lock.packages[""].name = pkg.name;
    }
  }
  writeText("package-lock.json", `${JSON.stringify(lock, null, 2)}\n`);
}

// Cargo.toml package.version (first version key after [package])
replaceInFile("Cargo.toml", (text) => {
  if (!/\[package\][\s\S]*?version = "[^"]+"/.test(text)) {
    fail("Cargo.toml: package version not found");
  }
  return text.replace(
    /(\[package\][\s\S]*?version = ")[^"]+(")/,
    `$1${newVersion}$2`,
  );
});

// harness-version markers
const markerRe = /(<!--\s*harness-version:\s*)([^\s-]+)(\s*-->)/;
for (const rel of ["templates/AGENTS.md", "AGENTS.md"]) {
  replaceInFile(rel, (text) => {
    if (!markerRe.test(text)) {
      console.warn(`bump-version: no harness-version marker in ${rel} (skipped)`);
      return text;
    }
    return text.replace(markerRe, `$1${newVersion}$3`);
  });
}

// Keep a Changelog: promote [Unreleased] → [newVersion] - date (US-038)
// Pure inline helper (keep in sync with src/application/changelog-hygiene.ts).
function promoteUnreleased(changelog, version, date) {
  const text = changelog.replace(/\r\n/g, "\n");
  const headingRe = /^##\s+\[([^\]]+)\][^\n]*/gm;
  /** @type {{ key: string; start: number; bodyStart: number }[]} */
  const heads = [];
  let m;
  while ((m = headingRe.exec(text)) !== null) {
    heads.push({
      key: m[1].trim(),
      start: m.index,
      bodyStart: m.index + m[0].length,
    });
  }
  /** @type {Map<string, { body: string; headingStart: number; end: number }>} */
  const sections = new Map();
  for (let i = 0; i < heads.length; i++) {
    const end = i + 1 < heads.length ? heads[i + 1].start : text.length;
    const body = text
      .slice(heads[i].bodyStart, end)
      .replace(/^\n+/, "")
      .replace(/\n+$/, "");
    sections.set(heads[i].key, {
      body,
      headingStart: heads[i].start,
      end,
    });
  }
  if (sections.has(version)) {
    return { text: changelog, promoted: false, reason: "already-versioned" };
  }
  const unreleased = sections.get("Unreleased");
  if (!unreleased || unreleased.body.trim().length === 0) {
    return { text: changelog, promoted: false, reason: "empty-unreleased" };
  }
  const body = unreleased.body.trim();
  const emptyUnreleased = "## [Unreleased]\n\n";
  const versionBlock = `## [${version}] - ${date}\n\n${body}\n\n`;
  const before = text.slice(0, unreleased.headingStart);
  const after = text.slice(unreleased.end).replace(/^\n+/, "");
  const next = `${before}${emptyUnreleased}${versionBlock}${after}`.replace(
    /\n{3,}/g,
    "\n\n",
  );
  return {
    text: next.endsWith("\n") ? next : `${next}\n`,
    promoted: true,
    reason: "ok",
  };
}

const changelogRel = "CHANGELOG.md";
const changelogText = readText(changelogRel, false);
if (changelogText !== null) {
  const today = new Date().toISOString().slice(0, 10);
  const cut = promoteUnreleased(changelogText, newVersion, today);
  if (cut.promoted) {
    writeText(changelogRel, cut.text);
    console.log(
      `bump-version: promoted CHANGELOG [Unreleased] → [${newVersion}] - ${today}`,
    );
  } else {
    console.log(
      `bump-version: CHANGELOG Unreleased not promoted (${cut.reason})`,
    );
  }
} else {
  console.warn("bump-version: CHANGELOG.md missing (skipped promote)");
}

console.log(`bumped ${oldVersion} → ${newVersion} (${["patch", "minor", "major"].includes(arg) ? arg : "exact"})`);
// CI-friendly last line
console.log(newVersion);
