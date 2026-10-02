// Gera site/src/fluent-icons.css com os ícones do site, tirados dos Fluent UI
// System Icons (pacote oficial `@fluentui/svg-icons`, licença MIT, © Microsoft)
// — o mesmo conjunto que o app usa via `@fluentui/react-icons`.
//
// Cada ícone vira uma classe `fi-<nome>` (com a base `fi`): o SVG entra como
// máscara pintada com `currentColor`, então o ícone herda a cor do texto e o
// tamanho vem do `font-size` (1em). Só os ícones listados abaixo entram.
//
// Uso (da raiz do repositório):
//   node site/scripts/fluent-icons.mjs            baixa o pacote com `npm pack`
//   node site/scripts/fluent-icons.mjs <pasta>    usa um pacote já extraído
// Depois: `pnpm --filter reemu-site build`.
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const VERSION = "1.1.343";

// classe → arquivo (sem `_24_regular.svg`)
const ICONS = {
  library: "library",
  games: "games",
  "paint-brush": "paint_brush",
  save: "save",
  heart: "heart",
  timer: "timer",
  image: "image",
  apps: "apps",
  controller: "xbox_controller",
  history: "history",
  "arrow-sync": "arrow_sync",
  "arrow-download": "arrow_download",
  star: "star",
  bug: "bug",
  share: "share",
  "qr-code": "qr_code",
  coffee: "drink_coffee",
  payment: "payment",
};

let pkg = process.argv[2];
if (!pkg) {
  const dir = mkdtempSync(join(tmpdir(), "fluent-icons-"));
  execFileSync("npm", ["pack", `@fluentui/svg-icons@${VERSION}`, "--silent"], { cwd: dir });
  const tgz = readdirSync(dir).find((f) => f.endsWith(".tgz"));
  execFileSync("tar", ["xzf", tgz], { cwd: dir });
  pkg = join(dir, "package");
}

const out = [
  "/* Gerado por site/scripts/fluent-icons.mjs — não editar à mão.",
  `   Fluent UI System Icons (@fluentui/svg-icons ${VERSION}), licença MIT,`,
  "   © Microsoft Corporation. */",
  ".fi {",
  "  display: inline-block;",
  "  flex-shrink: 0;",
  "  width: 1em;",
  "  height: 1em;",
  "  vertical-align: -0.125em;",
  "  background-color: currentColor;",
  "  -webkit-mask: var(--fi) center / contain no-repeat;",
  "  mask: var(--fi) center / contain no-repeat;",
  "}",
];
for (const [cls, file] of Object.entries(ICONS)) {
  const path = join(pkg, "icons", `${file}_24_regular.svg`);
  if (!existsSync(path)) throw new Error(`ícone não encontrado: ${path}`);
  // a máscara só usa o alfa: a cor do traço não importa
  const svg = readFileSync(path, "utf8").trim().replace(/\s+/g, " ");
  const uri = `data:image/svg+xml,${encodeURIComponent(svg).replace(/'/g, "%27")}`;
  out.push(`.fi-${cls} { --fi: url("${uri}"); }`);
}
const site = join(dirname(fileURLToPath(import.meta.url)), "..");
writeFileSync(join(site, "src/fluent-icons.css"), out.join("\n") + "\n");
console.log(`src/fluent-icons.css ← ${Object.keys(ICONS).length} ícones (@fluentui/svg-icons ${VERSION})`);
