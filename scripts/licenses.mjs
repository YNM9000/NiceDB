import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = new URL("../", import.meta.url).pathname;
const ui = join(root, "app/ui");
const OUT = join(ui, "src/assets/licenses.json");
const BUNDLED_DEV = ["svelte", "@inlang/paraglide-js"];
const LICENCE_FILE = /^(licen[cs]e|copying|notice|unlicense|copyright)([-._].*)?$/i;
const MAX_TEXT = 64 * 1024;

const run = (cmd, args, cwd) => execFileSync(cmd, args, { cwd, encoding: "utf8", maxBuffer: 256 << 20, stdio: ["ignore", "pipe", "pipe"] });

export function normaliseLicence(expr) {
  const e = (expr ?? "").trim().replace(/\s*\/\s*/g, " OR ");
  if (!e) return "";
  if (/[()]/.test(e) || / AND /.test(e)) return e;
  return [...new Set(e.split(/ OR /))].sort().join(" OR ");
}

function licenceFiles(dir, declared) {
  const names = existsSync(dir) ? readdirSync(dir).filter((n) => LICENCE_FILE.test(n) && statSync(join(dir, n)).isFile()) : [];
  if (declared && existsSync(join(dir, declared)) && !names.includes(declared)) names.push(declared);
  return names.sort().map((n) => readFileSync(join(dir, n), "utf8").slice(0, MAX_TEXT));
}

const STANDARD = ["MIT", "Apache-2.0", "BSD-3-Clause", "BSD-2-Clause", "MPL-2.0", "CC0-1.0"];

export function standardText(expr, holder) {
  const ids = expr.split(/ OR | AND |[()]/).map((s) => s.trim());
  const id = STANDARD.find((s) => ids.includes(s));
  if (!id) return null;
  return readFileSync(join(root, "scripts/licenses", `${id}.txt`), "utf8").replace("{holder}", holder);
}

function withTexts(e, holder) {
  if (e.texts.length > 0) return e;
  const text = standardText(e.license, holder || `the ${e.name} authors`);
  return text ? { ...e, texts: [text], standard: true } : e;
}

function crates() {
  const meta = JSON.parse(run("cargo", ["metadata", "--format-version", "1", "--locked", "--offline"], root));
  const byId = new Map(meta.packages.map((p) => [p.id, p]));
  const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
  const members = new Set(meta.workspace_members);
  const app = meta.packages.find((p) => p.name === "nicedb-app");
  const seen = new Set();
  const todo = [app.id];
  while (todo.length) {
    const id = todo.pop();
    if (seen.has(id)) continue;
    seen.add(id);
    for (const d of nodes.get(id)?.deps ?? []) {
      if (d.dep_kinds.some((k) => k.kind === null)) todo.push(d.pkg);
    }
  }
  return [...seen]
    .filter((id) => !members.has(id))
    .map((id) => byId.get(id))
    .map((p) => {
      const dir = p.manifest_path.replace(/\/Cargo\.toml$/, "");
      const holder = p.authors.map((a) => a.replace(/\s*<[^>]*>/, "")).join(", ");
      return withTexts({ source: "crate", name: p.name, version: p.version, license: normaliseLicence(p.license), url: p.repository ?? p.homepage ?? "", texts: licenceFiles(dir, p.license_file) }, holder);
    });
}

function npm() {
  const list = JSON.parse(run("pnpm", ["licenses", "list", "--prod", "--json"], ui));
  const out = [];
  for (const group of Object.values(list)) {
    for (const p of group) {
      p.versions.forEach((version, i) => {
        const holder = typeof p.author === "string" ? p.author.replace(/\s*<[^>]*>/, "") : "";
        out.push(withTexts({ source: "npm", name: p.name, version, license: normaliseLicence(p.license), url: p.homepage ?? "", texts: licenceFiles(p.paths[i] ?? p.paths[0], null) }, holder));
      });
    }
  }
  for (const name of BUNDLED_DEV) {
    const dir = join(ui, "node_modules", name);
    const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    const url = typeof pkg.repository === "string" ? pkg.repository : (pkg.repository?.url ?? pkg.homepage ?? "");
    out.push({ source: "npm", name, version: pkg.version, license: normaliseLicence(pkg.license), url: url.replace(/^git\+/, "").replace(/\.git$/, ""), texts: licenceFiles(dir, null) });
  }
  return out;
}

function assets() {
  const ofl = readFileSync(join(root, "scripts/licenses/OFL-1.1.txt"), "utf8");
  const cc0 = readFileSync(join(root, "scripts/licenses/CC0-1.0.txt"), "utf8");
  const apache = readFileSync(join(root, "scripts/licenses/Apache-2.0.txt"), "utf8");
  const sounds = readFileSync(join(ui, "src/assets/sounds/SOURCES.md"), "utf8");
  const fonts = readFileSync(join(ui, "src/styles/SOURCES.md"), "utf8");
  const marks = readFileSync(join(ui, "src/assets/marks/SOURCES.md"), "utf8");
  const font = (name, url) => {
    if (!fonts.includes(name)) throw new Error(`styles/SOURCES.md no longer lists ${name}`);
    return { source: "font", name, version: "", license: "OFL-1.1", url, texts: [ofl] };
  };
  return [
    font("IBM Plex Sans", "https://github.com/IBM/plex"),
    font("IBM Plex Sans Condensed", "https://github.com/IBM/plex"),
    font("IBM Plex Mono", "https://github.com/IBM/plex"),
    font("Source Serif 4", "https://github.com/adobe-fonts/source-serif"),
    { source: "sound", name: "NiceDB sounds (Marimba, hand chimes)", version: "", license: "CC0-1.0", url: "https://github.com/sgossner/VCSL", texts: [sounds.replace(/^# Sounds\n+/, ""), cc0] },
    { source: "icon", name: "devicon (engine marks)", version: "2.18.0", license: "MIT", url: "https://github.com/devicons/devicon", texts: [marks.replace(/^# Engine marks\n+/, "")] },
    { source: "icon", name: "Material Design Icons (ODBC mark)", version: "7.4.47", license: "Apache-2.0", url: "https://github.com/Templarian/MaterialDesign", texts: [apache] },
  ];
}

export function build(all) {
  const texts = {};
  const entries = [...new Map(all.map((e) => [key(e), e])).values()];
  const keyed = entries.map((e) => {
    const ids = e.texts.map((t) => {
      const clean = t.replace(/\r\n?/g, "\n").replace(/[ \t]+$/gm, "").trim();
      const id = createHash("sha256").update(clean.replace(/\s+/g, " ")).digest("hex").slice(0, 12);
      texts[id] ??= clean;
      return id;
    });
    return { source: e.source, name: e.name, version: e.version, license: e.license || "unknown", url: e.url, texts: [...new Set(ids)], ...(e.standard ? { standard: true } : {}) };
  });
  keyed.sort((a, b) => a.license.localeCompare(b.license) || a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
  const app = { license: "GPL-3.0-or-later", text: readFileSync(join(root, "LICENSE"), "utf8").trim() };
  return { app, entries: keyed, texts };
}

export const key = (e) => `${e.source}:${e.name}@${e.version}`;

function main() {
  const entries = [...new Map([...crates(), ...npm(), ...assets()].map((e) => [key(e), e])).values()];
  const unlicensed = entries.filter((e) => !e.license || e.texts.length === 0).map(key);
  if (process.argv.includes("--check")) {
    const saved = JSON.parse(readFileSync(OUT, "utf8"));
    const have = new Set(saved.entries.map(key));
    const missing = entries.map(key).filter((k) => !have.has(k));
    const empty = saved.entries.filter((e) => e.texts.length === 0 || e.texts.some((t) => !saved.texts[t])).map(key);
    if (missing.length || empty.length) {
      if (missing.length) console.error(`no licence entry (run node scripts/licenses.mjs):\n  ${missing.join("\n  ")}`);
      if (empty.length) console.error(`entries without a licence text:\n  ${empty.join("\n  ")}`);
      process.exit(1);
    }
    console.log(`licences: ${saved.entries.length} entries, all shipped packages covered`);
    return;
  }
  if (unlicensed.length) {
    console.error(`no licence file found:\n  ${unlicensed.join("\n  ")}`);
    process.exit(1);
  }
  const data = build(entries);
  writeFileSync(OUT, `${JSON.stringify(data)}\n`);
  const kb = Math.round(statSync(OUT).size / 1024);
  console.log(`${OUT}: ${data.entries.length} entries, ${Object.keys(data.texts).length} texts, ${kb} KB`);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
