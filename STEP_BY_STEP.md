# Passo a Passo — Setup Local do ReEmu

Pré-requisito: extrair `reemu-scaffold.tar.gz` num diretório
local antes de começar. Todos os comandos abaixo assumem que você está
dentro da pasta `reemu/`.

**Antes de tudo**: confira `TASKS.md` — é o checklist de progresso do
projeto. Ao delegar trabalho pra uma IA (Claude Code ou outro), aponte
pra ele primeiro ("veja o TASKS.md e continue da próxima etapa `todo`").
Atualize o status lá ao final de cada etapa concluída.

---

## 1. Instalar ferramentas base

```bash
# Rust (se ainda não tiver)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node + pnpm
# (via nvm, ou o gerenciador que preferir)
npm install -g pnpm

# Tauri CLI
cargo install tauri-cli --version "^2"

# Dependências de sistema do Tauri v2 (Linux) — varia por distro,
# ver: https://v2.tauri.app/start/prerequisites/
```

No Windows/macOS, siga os pré-requisitos específicos de plataforma na
documentação oficial do Tauri v2 (link acima) antes de continuar.

---

## 2. Validar o workspace Rust

```bash
cargo check --workspace
```

Isso deve compilar `domain` e `db` sem erros — são os dois únicos crates
com implementação real no scaffold. Se der erro de dependência, rode
`cargo update` primeiro.

---

## 3. Inicializar o app desktop (Tauri v2)

```bash
cd apps/desktop
cargo tauri init
```

Durante o wizard, use:
- **App name**: `ReEmu`
- **Frontend dev server**: `http://localhost:1420` (padrão Vite)
- **Frontend dist**: `../../packages/app-desktop/dist`
- **Dev command**: `pnpm --filter app-desktop dev`
- **Build command**: `pnpm --filter app-desktop build`

Depois de inicializado, edite `src-tauri/Cargo.toml` pra adicionar as
dependências dos crates internos:

```toml
[dependencies]
domain = { path = "../../../crates/domain" }
db = { path = "../../../crates/db" }
```

---

## 4. Criar o app React (frontend desktop)

```bash
cd packages/app-desktop
pnpm create vite@latest . -- --template react-ts
pnpm add zustand @tanstack/react-query
pnpm add @fluentui/react-components @fluentui/react-icons
```

Confirme que o `vite.config.ts` gerado usa a porta `1420` (padrão que o
Tauri espera) — ajuste se o wizard do passo 3 usou outra.

---

## 5. Instalar dependências na raiz do monorepo

```bash
cd ../..   # volta pra raiz do monorepo
pnpm install
```

---

## 6. Rodar em modo dev

```bash
cd apps/desktop
cargo tauri dev
```

Se tudo estiver certo, isso abre a janela do Tauri carregando o app React
vazio — ainda sem nenhuma feature implementada, só a fundação rodando.

---

## 7. Criar o primeiro crate adapter: `core-loader-desktop`

```bash
cd crates
cargo new core-loader-desktop --lib
cd core-loader-desktop
cargo add domain --path ../domain
cargo add libloading
cargo add gilrs
cargo add cpal
```

Adicione `"crates/core-loader-desktop"` na lista de `members` do
`Cargo.toml` raiz.

**A partir daqui, siga a ordem dos documentos em `docs/ai-context/`** —
cada um cobre uma etapa específica com as decisões de arquitetura já
resolvidas, pra usar como contexto ao trabalhar com um assistente de IA
(Claude Code ou outro).

---

## 8. Setup Android (só quando chegar nessa fase)

```bash
cd apps/mobile
cargo tauri init
cargo tauri android init
```

Requer Android SDK + NDK instalados. Lembre-se: `targetSdkVersion` deve
ser configurado baixo (ex: 28) no `AndroidManifest.xml` gerado, conforme
decidido — ver `docs/ai-context/11-android-port.md`.

---

## Ordem recomendada de implementação

1. `crates/db` — implementar repositórios reais (sqlx) sobre a migration existente
2. `crates/core-loader-desktop` — caminho GL, validado com um core software-only (ex: core de NES) antes de partir pra HW render
3. `apps/desktop` + `packages/app-desktop` — wiring básico, sem features ainda
4. Camadas de feature, uma por vez, seguindo `docs/ai-context/`
5. `apps/mobile` — só depois do desktop estar funcional ponta a ponta
