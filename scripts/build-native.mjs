#!/usr/bin/env node
/**
 * Build the Rust CLI and stage the current-platform binary + npm shim.
 */
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function fail(msg) {
  console.error(`build-native: ${msg}`);
  process.exit(1);
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
        fail(`refusing ${label} through symlink: ${path.relative(root, current)}`);
      }
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
  }
}

function assertRegularFile(file, label) {
  assertNoSymlinkComponents(file, label);
  let stat;
  try {
    stat = fs.lstatSync(file);
  } catch (error) {
    fail(`${label} is missing: ${file} (${error.message})`);
  }
  if (!stat.isFile() || stat.isSymbolicLink()) {
    fail(`${label} must be a regular file: ${file}`);
  }
}

function run(cmd, args) {
  const r = spawnSync(cmd, args, {
    cwd: root,
    stdio: "inherit",
    shell: false,
  });
  if (r.status !== 0) fail(`${cmd} ${args.join(" ")} exited ${r.status}`);
}

function rustTriple() {
  const p = process.platform;
  const a = process.arch;
  if (p === "win32" && a === "x64") return "x86_64-pc-windows-msvc";
  if (p === "win32" && a === "arm64") return "aarch64-pc-windows-msvc";
  if (p === "darwin" && a === "x64") return "x86_64-apple-darwin";
  if (p === "darwin" && a === "arm64") return "aarch64-apple-darwin";
  if (p === "linux" && a === "x64") return "x86_64-unknown-linux-gnu";
  if (p === "linux" && a === "arm64") return "aarch64-unknown-linux-gnu";
  fail(`unsupported platform ${p}-${a}`);
}

run("cargo", ["build", "--release"]);

const ext = process.platform === "win32" ? ".exe" : "";
const built = path.join(root, "target", "release", `harness${ext}`);
assertRegularFile(built, "built native binary");

const binDir = path.join(root, "bin");
const distDir = path.join(root, "dist");
assertNoSymlinkComponents(binDir, "binary directory");
assertNoSymlinkComponents(distDir, "dist directory");
fs.mkdirSync(binDir, { recursive: true });
assertNoSymlinkComponents(binDir, "binary directory");
fs.rmSync(distDir, { recursive: true, force: true });
fs.mkdirSync(distDir, { recursive: true });
assertNoSymlinkComponents(distDir, "dist directory");

const stagedName = `harness-${rustTriple()}${ext}`;
const staged = path.join(binDir, stagedName);
const generic = path.join(binDir, `harness${ext}`);
for (const destination of [staged, generic]) {
  assertNoSymlinkComponents(destination, "binary destination");
  if (fs.existsSync(destination) && fs.lstatSync(destination).isSymbolicLink()) {
    fail(`refusing to replace symlinked binary destination: ${destination}`);
  }
  fs.copyFileSync(built, destination);
  assertRegularFile(destination, "staged native binary");
}

const shimSrc = path.join(root, "npm", "shim.mjs");
const shimDest = path.join(distDir, "cli.js");
assertRegularFile(shimSrc, "npm launcher source");
assertNoSymlinkComponents(shimDest, "launcher destination");
if (fs.existsSync(shimDest) && fs.lstatSync(shimDest).isSymbolicLink()) {
  fail(`refusing to replace symlinked launcher destination: ${shimDest}`);
}
let shim = fs.readFileSync(shimSrc, "utf8");
if (!shim.startsWith("#!/usr/bin/env node")) {
  shim = `#!/usr/bin/env node\n${shim}`;
}
fs.writeFileSync(shimDest, shim);
assertRegularFile(shimDest, "launcher destination");
try {
  fs.chmodSync(shimDest, 0o755);
} catch {
  // Windows
}

console.log(`build-native: staged ${stagedName} and dist/cli.js`);
