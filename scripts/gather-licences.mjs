#!/usr/bin/env node
// Gathers the licences of every library compiled into the app and writes them
// to apps/desktop/src-tauri/licences/THIRD-PARTY.txt, which the app shows
// under Acknowledgements and carries in its bundle.
//
// "Compiled into the app" means: the Rust crates the desktop core and the
// native messaging host link on macOS (both architectures; not build scripts,
// macros or test helpers, which are not in the shipped program), and the
// JavaScript packages bundled into the window.
//
// Usage:
//   node scripts/gather-licences.mjs           write the file
//   node scripts/gather-licences.mjs --check   fail if the file is out of date
//
// Run it after changing dependencies. CI runs the check. Needs cargo and pnpm,
// with dependencies installed and fetched.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const OUT = join(ROOT, "apps/desktop/src-tauri/licences/THIRD-PARTY.txt");
const TARGETS = ["aarch64-apple-darwin", "x86_64-apple-darwin"];
/** The programs we ship: everything they link is in the app. */
const RUST_ROOTS = ["mbiro-desktop", "mbiro-nm-host"];
/** The workspace packages whose dependencies end up in the window. */
const JS_ROOTS = ["@mbiro/desktop", "@mbiro/protocol"];
/** Type declarations are not in the built window; the fonts have their own entries. */
const JS_SKIP = [/^@types\//, /^csstype$/, /^@fontsource\//, /^@mbiro\//];
const LICENCE_FILE = /^(licen[cs]e|copying|notice|unlicense|copyright)/i;

const run = (command, args) =>
  execFileSync(command, args, { cwd: ROOT, encoding: "utf8", maxBuffer: 256 * 1024 * 1024 });

/** Every licence-like file at the top of a package, as text. */
function licenceTexts(dir, named) {
  const names = readdirSync(dir).filter(
    (name) => LICENCE_FILE.test(name) && statSync(join(dir, name)).isFile(),
  );
  if (named && existsSync(join(dir, named)) && !names.includes(named)) names.push(named);
  return names
    .sort()
    .map((name) => readFileSync(join(dir, name), "utf8").replace(/\r\n/g, "\n").trim())
    .filter((text) => text.length > 0);
}

function rustLibraries() {
  const found = new Map();
  for (const target of TARGETS) {
    const metadata = JSON.parse(
      run("cargo", ["metadata", "--format-version", "1", "--locked", "--filter-platform", target]),
    );
    const packages = new Map(metadata.packages.map((p) => [p.id, p]));
    const nodes = new Map(metadata.resolve.nodes.map((n) => [n.id, n]));
    const queue = metadata.packages.filter((p) => RUST_ROOTS.includes(p.name)).map((p) => p.id);
    if (queue.length !== RUST_ROOTS.length) throw new Error("a Rust root package was not found");
    const seen = new Set(queue);
    while (queue.length > 0) {
      const node = nodes.get(queue.pop());
      for (const dep of node.deps) {
        // Only what is linked: a dependency of kind "build" or "dev" is not shipped.
        if (!dep.dep_kinds.some((kind) => kind.kind === null)) continue;
        if (seen.has(dep.pkg)) continue;
        seen.add(dep.pkg);
        queue.push(dep.pkg);
      }
    }
    for (const id of seen) {
      const p = packages.get(id);
      if (p.source === null) continue; // ours
      const dir = dirname(p.manifest_path);
      found.set(`${p.name} ${p.version}`, {
        name: p.name,
        version: p.version,
        licence: p.license ?? "see the licence text",
        home: p.repository ?? p.homepage ?? `https://crates.io/crates/${p.name}`,
        texts: licenceTexts(dir, p.license_file),
      });
    }
  }
  return [...found.values()];
}

function jsLibraries() {
  const found = new Map();
  const filters = JS_ROOTS.flatMap((name) => ["--filter", name]);
  const listed = JSON.parse(run("pnpm", ["licenses", "list", "--prod", "--json", ...filters]));
  for (const [licence, entries] of Object.entries(listed)) {
    for (const entry of entries) {
      if (JS_SKIP.some((skip) => skip.test(entry.name))) continue;
      entry.versions.forEach((version, index) => {
        found.set(`${entry.name} ${version}`, {
          name: entry.name,
          version,
          licence,
          home: entry.homepage ?? `https://www.npmjs.com/package/${entry.name}`,
          texts: licenceTexts(entry.paths[index] ?? entry.paths[0]),
        });
      });
    }
  }
  return [...found.values()];
}

const byName = (a, b) =>
  a.name.localeCompare(b.name, "en") || a.version.localeCompare(b.version, "en");

function render(rust, js) {
  const lines = [
    "THIRD-PARTY LIBRARIES",
    "",
    "The libraries compiled into this app, and the licences they are used under.",
    "Written by scripts/gather-licences.mjs; do not edit by hand.",
    "",
  ];
  const list = (title, libraries) => {
    lines.push(`${title} (${libraries.length})`, "");
    for (const l of libraries) lines.push(`  ${l.name} ${l.version}  [${l.licence}]  ${l.home}`);
    lines.push("");
  };
  list("RUST LIBRARIES", rust);
  list("JAVASCRIPT LIBRARIES", js);

  // Many libraries carry word-for-word the same licence: each text appears
  // once, under the names of everything that uses it.
  const texts = new Map();
  const without = [];
  for (const library of [...rust, ...js]) {
    if (library.texts.length === 0) without.push(library);
    for (const text of library.texts) {
      const key = createHash("sha256").update(text).digest("hex");
      const entry = texts.get(key) ?? { text, users: [] };
      entry.users.push(`${library.name} ${library.version}`);
      texts.set(key, entry);
    }
  }
  if (without.length > 0) {
    lines.push(
      "LIBRARIES WHOSE PACKAGE CARRIES NO LICENCE FILE",
      "",
      "Each is used under the licence named beside it; its text is at the address given.",
      "",
    );
    for (const l of without) lines.push(`  ${l.name} ${l.version}  [${l.licence}]  ${l.home}`);
    lines.push("");
  }
  lines.push("LICENCE TEXTS", "");
  const ordered = [...texts.values()].sort(
    (a, b) => b.users.length - a.users.length || a.users[0].localeCompare(b.users[0], "en"),
  );
  for (const { text, users } of ordered) {
    lines.push("=".repeat(78), `Used by: ${users.join(", ")}`, "=".repeat(78), "", text, "");
  }
  return lines.join("\n");
}

const rust = rustLibraries().sort(byName);
const js = jsLibraries().sort(byName);
const output = render(rust, js);

if (process.argv.includes("--check")) {
  const current = existsSync(OUT) ? readFileSync(OUT, "utf8") : "";
  if (current !== output) {
    console.error(
      "error: the library licence list is out of date. Run: node scripts/gather-licences.mjs",
    );
    process.exit(1);
  }
  console.log(`up to date: ${rust.length} Rust and ${js.length} JavaScript libraries`);
} else {
  writeFileSync(OUT, output);
  const missing = [...rust, ...js].filter((l) => l.texts.length === 0).length;
  console.log(
    `${rust.length} Rust and ${js.length} JavaScript libraries, ${missing} without a licence file`,
  );
}
