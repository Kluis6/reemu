#!/usr/bin/env node
// Resumo das licenças das bibliotecas que vão no ReEmu instalado, pra aba
// Configurações › Sistema › Informações e licenças. Grava
// packages/app-desktop/src/data/licenses.json — rode de novo ao mudar
// dependências:
//
//   node scripts/licenses-summary.mjs
//
// Rust: `cargo metadata`, só as dependências normais (sem dev/build) dos dois
// executáveis que vão no pacote (o app e o reemu-core-host), no Linux e no
// Windows. JavaScript: `pnpm licenses list --prod` do frontend.
import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const run = (cmd, args, cwd = root) =>
  execFileSync(cmd, args, { cwd, encoding: "utf8", maxBuffer: 256 << 20 });

// Executáveis distribuídos: o app (apps/desktop/src-tauri) e o core-host.
const BINARIES = ["reemu-desktop", "core-host-desktop"];
const TARGETS = ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"];

// "Apache-2.0 OR MIT" e "MIT/Apache-2.0" contam como a mesma licença.
function normalize(expr) {
  if (!expr) return "?";
  const e = expr.replaceAll("/", " OR ").replace(/\s+/g, " ").trim();
  if (/\(|\bAND\b|\bWITH\b/.test(e)) return e;
  return [...new Set(e.split(" OR "))].sort().join(" OR ");
}

function rustCrates() {
  const crates = new Map();
  for (const target of TARGETS) {
    const meta = JSON.parse(
      run("cargo", ["metadata", "--format-version", "1", "--locked", "--filter-platform", target]),
    );
    const pkgs = new Map(meta.packages.map((p) => [p.id, p]));
    const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
    const workspace = new Set(meta.workspace_members);
    const stack = meta.packages.filter((p) => BINARIES.includes(p.name)).map((p) => p.id);
    const seen = new Set();
    while (stack.length) {
      const id = stack.pop();
      if (seen.has(id)) continue;
      seen.add(id);
      const p = pkgs.get(id);
      if (!workspace.has(id)) crates.set(`${p.name}@${p.version}`, normalize(p.license));
      for (const d of nodes.get(id).deps) {
        // só dependências normais (kind null), que entram no executável
        if (d.dep_kinds.some((k) => k.kind === null)) stack.push(d.pkg);
      }
    }
  }
  return crates;
}

function jsPackages() {
  const out = JSON.parse(
    run("pnpm", ["licenses", "list", "--prod", "--json"], join(root, "packages/app-desktop")),
  );
  const pkgs = new Map();
  for (const [license, list] of Object.entries(out)) {
    for (const p of list) for (const v of p.versions) pkgs.set(`${p.name}@${v}`, normalize(license));
  }
  return pkgs;
}

function summary(map) {
  const byLicense = {};
  for (const lic of map.values()) byLicense[lic] = (byLicense[lic] ?? 0) + 1;
  const sorted = Object.entries(byLicense).sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
  return { total: map.size, byLicense: Object.fromEntries(sorted) };
}

const data = { rust: summary(rustCrates()), js: summary(jsPackages()) };
const file = join(root, "packages/app-desktop/src/data/licenses.json");
mkdirSync(dirname(file), { recursive: true });
writeFileSync(file, JSON.stringify(data, null, 2) + "\n");
console.log(`Rust: ${data.rust.total} crates · JS: ${data.js.total} pacotes → ${file}`);
