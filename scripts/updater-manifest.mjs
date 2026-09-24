#!/usr/bin/env node
// Monta o `latest.json` do auto-update (tauri-plugin-updater) a partir das
// assinaturas `.sig` que o `cargo tauri build` gera ao lado de cada
// instalador. Roda no release.yml quando a Release é PUBLICADA — assim as
// notas são o texto final da Release.
//
// Uso: node scripts/updater-manifest.mjs <versão> <tag> <repo> <dir-dos-sig> <notas.md>
// Saída: JSON no stdout. Sai com código 2 se não houver nenhuma assinatura.
//
// Chaves de plataforma que o plugin procura, nesta ordem:
// `{os}-{arch}-{instalador}` e depois `{os}-{arch}` (updater.rs, get_urls).
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const [version, tag, repo, sigDir, notesFile] = process.argv.slice(2);
if (!version || !tag || !repo || !sigDir || !notesFile) {
  console.error("uso: updater-manifest.mjs <versão> <tag> <repo> <dir-dos-sig> <notas.md>");
  process.exit(1);
}

const ARCH = { amd64: "x86_64", x64: "x86_64", x86_64: "x86_64", aarch64: "aarch64", arm64: "aarch64" };

// sufixo do instalador → [os, instalador, é o padrão do sistema?]
const KINDS = [
  [/\.AppImage$/, "linux", "appimage", true],
  [/\.deb$/, "linux", "deb", false],
  [/\.rpm$/, "linux", "rpm", false],
  [/-setup\.exe$/, "windows", "nsis", true],
  [/\.msi$/, "windows", "msi", false],
];

/** Exportado pro teste. */
export function platformsFor(files, readSig, baseUrl) {
  const platforms = {};
  for (const sigName of files.filter((f) => f.endsWith(".sig")).sort()) {
    const asset = sigName.slice(0, -".sig".length);
    const kind = KINDS.find(([re]) => re.test(asset));
    if (!kind) continue;
    const [, os, installer, isDefault] = kind;
    const archToken = asset.match(/_(amd64|x64|x86_64|aarch64|arm64)[_.-]/)?.[1];
    const arch = ARCH[archToken];
    if (!arch) continue;
    const entry = {
      signature: readSig(sigName).trim(),
      url: `${baseUrl}/${encodeURIComponent(asset)}`,
    };
    platforms[`${os}-${arch}-${installer}`] = entry;
    if (isDefault) platforms[`${os}-${arch}`] = entry;
  }
  return platforms;
}

const isMain = import.meta.url === `file://${process.argv[1]}`;
if (isMain) {
  const platforms = platformsFor(
    readdirSync(sigDir),
    (f) => readFileSync(join(sigDir, f), "utf8"),
    `https://github.com/${repo}/releases/download/${encodeURIComponent(tag)}`,
  );
  if (Object.keys(platforms).length === 0) {
    console.error("updater-manifest: nenhuma assinatura .sig reconhecida — sem latest.json");
    process.exit(2);
  }
  const manifest = {
    version,
    notes: readFileSync(notesFile, "utf8").trim(),
    pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    platforms,
  };
  process.stdout.write(JSON.stringify(manifest, null, 2) + "\n");
}
