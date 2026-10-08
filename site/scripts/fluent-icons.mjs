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
  controller: "xbox_controller",
  "arrow-sync": "arrow_sync",
  "puzzle-piece": "puzzle_piece",
  "developer-board": "developer_board",
  "arrow-download": "arrow_download",
  star: "star",
  bug: "bug",
  share: "share",
  mail: "mail",
  community: "people_community",
  "qr-code": "qr_code",
  coffee: "drink_coffee",
  payment: "payment",
};

// Ícones desenhados pro ReEmu no mesmo estilo do Fluent (grade 24×24, traço
// 1,5, cantos arredondados) — o conjunto da Microsoft não tem estes.
const CUSTOM = {
  // Joystick estilo Atari (CX40): caixa em perspectiva 3/4, botão de tiro no
  // canto de trás à esquerda, bastão com pega no centro.
  joystick:
    '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6.25 12.25h11.5L21 16v3.25A1.75 1.75 0 0 1 19.25 21H4.75A1.75 1.75 0 0 1 3 19.25V16z"/><path d="M3 16h18"/><ellipse cx="7.25" cy="14.1" rx="1.4" ry=".6"/><path d="M12 14.1V9.5"/><rect x="10" y="2.75" width="4" height="6.75" rx="2"/></svg>',
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
// a máscara só usa o alfa: a cor do traço não importa
const rule = (cls, svg) => {
  const uri = `data:image/svg+xml,${encodeURIComponent(svg.trim().replace(/\s+/g, " ")).replace(/'/g, "%27")}`;
  out.push(`.fi-${cls} { --fi: url("${uri}"); }`);
};
for (const [cls, file] of Object.entries(ICONS)) {
  const path = join(pkg, "icons", `${file}_24_regular.svg`);
  if (!existsSync(path)) throw new Error(`ícone não encontrado: ${path}`);
  rule(cls, readFileSync(path, "utf8"));
}
out.push("/* Desenhados pro ReEmu (mesmo estilo; não fazem parte do conjunto da Microsoft). */");
for (const [cls, svg] of Object.entries(CUSTOM)) rule(cls, svg);
const site = join(dirname(fileURLToPath(import.meta.url)), "..");
writeFileSync(join(site, "src/fluent-icons.css"), out.join("\n") + "\n");
console.log(
  `src/fluent-icons.css ← ${Object.keys(ICONS).length} ícones (@fluentui/svg-icons ${VERSION}) + ${Object.keys(CUSTOM).length} próprio(s)`,
);
