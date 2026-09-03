# Docs de Contexto para IA

Cada arquivo aqui cobre uma etapa do projeto e foi escrito pra ser colado
(ou referenciado) no início de uma sessão com assistente de IA (Claude Code
ou outro) ao trabalhar especificamente naquela parte — sem precisar
reexplicar o projeto inteiro toda vez.

**Sempre inclua `00-visao-geral.md` junto do documento da etapa atual.**
Os demais são independentes entre si — não precisa incluir todos de uma vez.

**Confira `../../TASKS.md` (raiz do repo) antes de começar** — ele diz qual
etapa está `todo`/`in-progress`/`done`, evitando retrabalho ou pular
pré-requisito entre sessões.

**`REFERENCES.md`** — links da documentação oficial externa (libretro, Tauri,
wgpu, Fluent, Wayland/EGL). Regra do projeto: consultar a fonte oficial antes
de escrever binding FFI / usar API de terceiro / depurar plataforma — não
trabalhar de memória (ver `00-visao-geral.md`).

## Ordem recomendada

| Doc | Etapa | Depende de |
|---|---|---|
| `00-visao-geral.md` | Contexto geral (sempre incluir) | — |
| `01-domain-db.md` | Implementar repositórios SQLite | domain, db (já scaffolded) |
| `02-core-loader-desktop.md` | Carregamento de core, caminho GL | 01 |
| `03-tauri-desktop-shell.md` | App Tauri, surface nativa de vídeo | 02 |
| `04-shader-chain-decoracao.md` | Filtros de imagem, Mega Bezel, bezels | 03 |
| `05-input-hotkeys.md` | Reconhecimento de controle, hotkeys, binding | 03 |
| `06-audio.md` | Saída de áudio, Dynamic Rate Control | 03 |
| `07-frontend-react.md` | UI Fluent, overlay, foco, toast, Zustand | 03 |
| `08-save-states.md` | Save state e save RAM | 02 |
| `09-metadata-scraping.md` | Scraping de biblioteca | 01 |
| `10-core-catalog.md` | Listagem/download de cores | 01 |
| `11-android-port.md` | Port pra Android | tudo acima, no desktop |
| `12-vulkan-hw-render-fase2.md` | HW render Vulkan por-core (backlog) | 02 |

Docs 04 a 10 podem ser feitos em paralelo/qualquer ordem entre si, desde
que 01-03 já estejam prontos.
