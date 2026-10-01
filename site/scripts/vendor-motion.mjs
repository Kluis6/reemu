// Copia o bundle UMD do Motion (versão travada no package.json) para
// vendor/, servido junto com o site. Roda no `build` e no `dev`.
import { copyFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const site = join(dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(join(site, "package.json"));
const motionDir = dirname(require.resolve("motion/package.json"));
copyFileSync(join(motionDir, "dist/motion.js"), join(site, "vendor/motion.js"));
console.log("vendor/motion.js ← motion/dist/motion.js");
