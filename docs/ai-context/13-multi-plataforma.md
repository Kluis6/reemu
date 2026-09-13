# 13 — Multi-plataforma (Linux hoje, Windows planejado)

## Alvo atual vs planejado

- **Hoje**: só Linux (Wayland, WebKitGTK), único hardware testado é NVIDIA
  (driver proprietário).
- **Planejado**: uma versão Windows (WebView2/Chromium via Tauri).
- macOS aparece em alguns `cfg(target_os = "macos")` só por completude (a
  API do Tauri/Rust pede tratar os 3 casos em `match`/`if`) — **não é alvo
  confirmado**, tratar como "não testado", nunca como "suportado".

## Regra geral daqui pra frente

Boa parte do que está documentado neste projeto como "regra" de performance/
rendering do frontend é na verdade **uma limitação do WebKitGTK + driver
NVIDIA proprietário no Linux**, não uma decisão de arquitetura universal.
Ao escrever um comentário, doc ou memória sobre uma limitação técnica,
perguntar: *"isso é do Tauri? do Fluent? ou é do WebKitGTK/Linux
especificamente?"* — só a última categoria precisa da tag platform-specific,
e precisa ser **reavaliada**, não carregada como dogma, quando o port pra
Windows começar.

## Catálogo do que já é platform-specific (levantado no código)

### Rust — já tem `cfg(target_os = ...)`

- **`apps/desktop/src-tauri/src/video.rs`**: o caminho de vídeo nativo hoje
  é um `wl_subsurface` (protocolo Wayland) — só Linux. Windows/macOS caem
  num "surface direto no handle da janela" que o PRÓPRIO comentário do
  arquivo já marca como `Não verificado` — isso é trabalho real pendente
  pro port, não só um detalhe de porte trivial. A razão de existir a
  subsurface (em vez de webview transparente simples) é um bug específico
  **WebKitGTK + NVIDIA + Wayland** (webview transparente não pinta) — não
  há motivo pra achar que o Windows/WebView2 vai precisar do mesmo
  contorno.
- **`apps/desktop/src-tauri/src/main.rs::configure_webkit_gpu`**: só
  Linux — desliga compositing do WebKitGTK especificamente quando detecta
  driver NVIDIA proprietário (`/proc/driver/nvidia/version`,
  `/dev/nvidia0`, `__GLX_VENDOR_LIBRARY_NAME`). **Não existe conceito
  equivalente no WebView2** (motor Chromium, história de compositing
  completamente diferente) — não portar esta função; ela não faz sentido
  fora de WebKitGTK.
- **`crates/core-loader-desktop/src/discover.rs`,
  `apps/desktop/src-tauri/src/core_catalog.rs`,
  `crates/core-loader-desktop/src/loader.rs`**: já resolvem extensão de
  dylib (`.so`/`.dll`/`.dylib`) e o nome do canal do buildbot da libretro
  por OS. Já preparado — quando o Windows entrar, é só VALIDAR essas
  branches, não escrever nada novo aqui.
- **`apps/desktop/src-tauri/Cargo.toml`**: `wayland-client` só entra como
  dependência no target Linux (`[target.'cfg(target_os = "linux")'.dependencies]`)
  — correto; só precisa de algo análogo pro Windows quando o vídeo nativo
  for implementado lá (hoje cai no fallback `<canvas>`, que já funciona em
  qualquer OS).

### Frontend — hoje ZERO separação (tudo tratado como regra universal)

- **"Nada de `backdrop-filter`"** (`styles/xbox.ts`, memória
  `frontend-perf-webkitgtk`): consequência DIRETA do mesmo bug
  WebKitGTK+NVIDIA acima, não uma escolha de design. O WebView2 (Chromium)
  tem suporte robusto e acelerado a `backdrop-filter`. **Quando o port pra
  Windows existir, esta restrição provavelmente pode ser LIGADA lá** — isso
  também reabre a decisão de "Acrylic descartado" da revisão Fluent 2 de
  material (ver [[fluent2-design-audit]]): lá eu descartei Acrylic como
  "não dá pra fazer" sem qualificar que isso vale só pro alvo Linux atual.
- **Animação só estática, nunca contínua** (`AnimatedBackground`, mesma
  memória): mesma raiz — sem compositing acelerado, todo `transform`/
  `opacity`/`filter` repinta na CPU a cada frame. No Windows isso não
  deveria ser um problema (WebView2 tem compositing de verdade sempre,
  independente de fabricante de GPU).
- Nenhuma dessas duas regras está atrás de uma checagem de plataforma no
  frontend hoje — são incondicionais. Quando o Windows existir, cada uma
  vira candidata a ficar condicional (o Tauri expõe a plataforma em tempo
  de execução via `@tauri-apps/plugin-os` — `platform()` — pro frontend
  decidir).

## O que É universal (não mexer nem quando o Windows entrar)

- Toda a arquitetura hexagonal (`domain` e a maioria dos crates) — sem
  dependência de plataforma nenhuma.
- Componentes/tokens/temas Fluent — elevação, cor, iconografia, layout,
  material já revisados nesta série (ver [[fluent2-design-audit]]) — Fluent
  2 é cross-platform por definição, essas decisões valem em qualquer OS.
- Lógica de negócio do frontend (React Query, stores Zustand, rotas).
- O modelo de domínio de emulação (`FrameSource`, `CoreLoader`,
  `GpuTextureHandle`) — o HW render GL/Vulkan em si não é Linux-specific
  (Windows tem os dois); só o *transporte* pro compositor (dma_buf vs. o
  que o Windows for usar) é que muda.

## Como aplicar isso indo em frente

- Nunca escrever "o projeto não faz X" quando o correto é "o WebKitGTK no
  Linux não faz X" — a diferença importa assim que existir um 2º alvo.
- Não implementar nada específico de Windows "no escuro" (sem máquina
  Windows pra testar) — documentar como CANDIDATO/pendência clara (como o
  próprio `video.rs` já faz com "Não verificado"), nunca fingir que está
  pronto.
- Ao revisar contra a doc do Fluent 2 (ver [[fluent2-design-audit]]), se a
  razão de descartar alguma coisa for "o WebKitGTK não aguenta", registrar
  explicitamente que a decisão é por-plataforma, não definitiva.
