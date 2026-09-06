#!/usr/bin/env node
/**
 * Extract GitHub Release notes from CHANGELOG.md for a given version.
 *
 * Usage:
 *   node scripts/release-notes.mjs [version] [--out path] [--with-export] [--since date]
 *
 * Prefers compiled dist/ when present (after release:check / build);
 * otherwise loads the TypeScript source via dynamic import of the pure
 * helpers inlined below (keep in sync with src/application/release-notes.ts).
 *
 * Resolution order for CHANGELOG body:
 *   1. ## [X.Y.Z] section (exact version)
 *   2. ## [Unreleased] section
 *   3. Fallback body with npm/GitHub links
 *
 * Optional `--with-export` (US-038) appends durable-history assist from
 * `harness export changelog` (implemented stories/decisions). Human
 * CHANGELOG remains the primary source of truth.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");

function failUnsafePath(label, file, error) {
  const detail = error?.message ? ` (${error.message})` : "";
  throw new Error(`release-notes: refusing unsafe ${label} path ${file}${detail}`);
}

function assertNoSymlinkComponents(file, label) {
  const resolved = path.resolve(file);
  const rootPath = path.parse(resolved).root;
  let current = rootPath;
  const components = resolved.slice(rootPath.length).split(path.sep).filter(Boolean);
  for (const part of components) {
    current = path.join(current, part);
    try {
      if (fs.lstatSync(current).isSymbolicLink()) {
        failUnsafePath(label, file);
      }
    } catch (error) {
      if (error?.code !== "ENOENT") failUnsafePath(label, file, error);
    }
  }
}

function writeOutput(file, content) {
  assertNoSymlinkComponents(file, "release output");
  try {
    if (fs.existsSync(file) && !fs.lstatSync(file).isFile()) {
      failUnsafePath("release output", file);
    }
    fs.writeFileSync(file, content, "utf8");
  } catch (error) {
    if (error instanceof Error && error.message.startsWith("release-notes:")) throw error;
    failUnsafePath("release output", file, error);
  }
}

/**
 * @param {string} changelog
 * @param {string} version
 * @returns {string | null}
 */
function extractChangelogSection(changelog, version) {
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
  /** @type {Map<string, string>} */
  const sections = new Map();
  for (let i = 0; i < heads.length; i++) {
    const end = i + 1 < heads.length ? heads[i + 1].start : text.length;
    const body = text.slice(heads[i].bodyStart, end).trim();
    sections.set(heads[i].key, body);
  }
  if (sections.has(version)) {
    const body = sections.get(version);
    if (body && body.length > 0) return body;
  }
  if (sections.has("Unreleased")) {
    const body = sections.get("Unreleased");
    if (body && body.length > 0) return body;
  }
  return null;
}

/**
 * @param {string} entriesMd
 * @returns {string}
 */
function formatExportAssistSectionFallback(entries) {
  if (!entries || entries.length === 0) return "";
  const lines = ["### From harness history (assist)", ""];
  for (const e of entries) {
    const label = e.type === "decision" ? "Decision" : "Story";
    lines.push(`- [${label}] ${e.id}: ${e.title}`);
  }
  lines.push("");
  return lines.join("\n");
}

/**
 * @param {{ version: string; packageName?: string; repoUrl?: string; changelogText?: string | null; exportEntries?: Array<{id:string;title:string;type:string}> | null }} opts
 * @returns {string}
 */
function buildReleaseNotes(opts) {
  const version = opts.version;
  const packageName = opts.packageName ?? "5harness";
  const repoUrl =
    opts.repoUrl ?? "https://github.com/vantanminh/5harness";

  let section = null;
  if (opts.changelogText) {
    section = extractChangelogSection(opts.changelogText, version);
  }

  const lines = [`## ${packageName} v${version}`, ""];

  if (section) {
    lines.push(section, "");
  } else {
    lines.push(
      `Release **${version}**.`,
      "",
      "See [CHANGELOG.md](./CHANGELOG.md) for details when available.",
      "",
    );
  }

  if (opts.exportEntries && opts.exportEntries.length > 0) {
    lines.push(
      formatExportAssistSectionFallback(opts.exportEntries).trimEnd(),
      "",
    );
  }

  lines.push(
    "### Install",
    "",
    "```bash",
    `npm i -g ${packageName}@${version}`,
    "```",
    "",
    "### Supply chain",
    "",
    "- Published via **npm trusted publishing** (OIDC) with **provenance** attestations when available.",
    `- Package: https://www.npmjs.com/package/${packageName}/v/${version}`,
    `- Tag: ${repoUrl}/releases/tag/v${version}`,
    "",
  );

  return lines.join("\n");
}

/**
 * Prefer dist/application/release-notes.js when built; else local helpers.
 * @returns {Promise<{ extractChangelogSection: typeof extractChangelogSection; buildReleaseNotes: typeof buildReleaseNotes }>}
 */
async function loadImpl() {
  const distPath = path.join(root, "dist", "application", "release-notes.js");
  if (fs.existsSync(distPath)) {
    return import(pathToFileURL(distPath).href);
  }
  return { extractChangelogSection, buildReleaseNotes };
}

/**
 * Load export-changelog entries from dist (fail-open).
 * @param {string | undefined} since
 * @returns {Promise<Array<{id:string;title:string;type:string}> | null>}
 */
async function loadExportEntries(since) {
  const distPath = path.join(root, "dist", "application", "export-changelog.js");
  if (!fs.existsSync(distPath)) {
    console.error(
      "release-notes: --with-export skipped (dist/application/export-changelog.js missing; run build)",
    );
    return null;
  }
  try {
    const mod = await import(pathToFileURL(distPath).href);
    const entries = mod.buildChangelog(root, since);
    return entries.map((e) => ({
      id: e.id,
      title: e.title,
      type: e.type,
    }));
  } catch (err) {
    console.error(
      `release-notes: --with-export failed (continuing without assist): ${err?.message ?? err}`,
    );
    return null;
  }
}

function parseArgs(argv) {
  let version = null;
  let out = null;
  let withExport = false;
  let since = undefined;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--out" || a === "-o") {
      out = argv[++i];
      continue;
    }
    if (a === "--with-export") {
      withExport = true;
      continue;
    }
    if (a === "--since") {
      since = argv[++i];
      continue;
    }
    if (a.startsWith("-")) {
      console.error(`Unknown flag: ${a}`);
      process.exit(2);
    }
    if (!version) version = a;
  }
  if (!version) {
    const pkg = JSON.parse(
      fs.readFileSync(path.join(root, "package.json"), "utf8"),
    );
    version = pkg.version;
  }
  return { version, out, withExport, since };
}

async function main() {
  const { version, out, withExport, since } = parseArgs(process.argv.slice(2));
  const pkg = JSON.parse(
    fs.readFileSync(path.join(root, "package.json"), "utf8"),
  );
  const changelogPath = path.join(root, "CHANGELOG.md");
  const changelogText = fs.existsSync(changelogPath)
    ? fs.readFileSync(changelogPath, "utf8")
    : null;

  let exportEntries = null;
  if (withExport) {
    exportEntries = await loadExportEntries(since);
  }

  const impl = await loadImpl();
  const notes = impl.buildReleaseNotes({
    version,
    packageName: pkg.name,
    repoUrl: "https://github.com/vantanminh/5harness",
    changelogText,
    exportEntries,
  });

  if (out) {
    writeOutput(out, notes);
    console.error(`Wrote release notes → ${out}`);
  } else {
    process.stdout.write(notes);
  }
}

const isMain =
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isMain) {
  main().catch((err) => {
    console.error(err);
    process.exit(1);
  });
}

export { extractChangelogSection, buildReleaseNotes };
