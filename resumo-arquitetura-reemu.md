# ReEmu — Resumo de Arquitetura

Documento consolidado de todas as decisões tomadas durante o design do projeto.

---

## 1. Stack e Fundação

- **Tauri v2 + React 19 + TypeScript + SQLite + Fluent Design**
- **Monorepo**, separando o que é compartilhado do que diverge por plataforma:

```
/apps
  /desktop            → projeto Tauri v2 (Windows + Linux)
  /mobile              → projeto Tauri v2 mobile (Android)

/crates (Cargo workspace)
  /domain              → regras de negócio puras, sem I/O de plataforma
  /db                  → sqlx/rusqlite, migrations, queries compartilhadas
  /core-loader-desktop → libloading + retro_* FFI, cpal, gilrs
  /core-loader-mobile  → cores empacotados, JNI/áudio nativo Android

/packages (pnpm workspace)
  /ui                  → componentes Fluent, tokens de design
  /shared              → hooks, cliente IPC, state management
  /app-desktop         → entrypoint React desktop
  /app-mobile          → entrypoint React mobile (layout touch)
```

- **Arquitetura hexagonal** (domain + adapters), **SOLID** como princípio orientador — sem MVC
- **Sem multi-perfil no MVP**: todas as configs (hotkeys, áudio, etc.) são únicas por instalação, sem `user_profile_id`
- Escopo: suporte amplo a cores (dezenas, estilo RetroArch), biblioteca grande (milhares de jogos)
- Documentações de referência: opengl.org, vulkan.org, gpuopen.com/vulkan, docs.libretro.com

---

## 2. Renderização (OpenGL + Vulkan)

- **Camada global** (`gpu-context`): instância/dispositivo Vulkan e contexto GL únicos por sessão, usados pelo pipeline de pós-processamento — inclusive por cores só-CPU (frame cru vira textura e entra no mesmo pipeline)
- **Camada por-core** (`hw-render-negotiator`): negociação de hardware render para cores que exigem (Dreamcast, PS2, N64, PS1 HW)
  - Implementação **GL primeiro** (fase 1) — cobre a maioria dos cores hw-accelerated existentes
  - **Vulkan por-core como fase 2** — adiado deliberadamente por ser a parte historicamente mais bugada (sincronização de VkImage entre core e frontend)
- Abstração `FrameSource`: unifica saída de core software e hardware como "textura + metadata", para que o pós-processamento nunca precise saber a origem do frame
- **Metadata técnica de core** (`render_backend`, `gl_version_min` etc.): detectada **em runtime**, no primeiro load do core (via `retro_hw_render_callback`) — sem curadoria manual

### Renderização fora da WebView
- O jogo renderiza numa **surface nativa própria** (fora da WebView do Tauri), visando 60fps com baixo input lag
- **Android diverge**: sem child windows como no desktop — usa `SurfaceView`/`TextureView` nativa via JNI

---

## 3. Overlay de Menu e Foco

### State machine de foco (`InputFocus`)
```
GameFocused ⇄ MenuFocused    (toggle via hotkey personalizável)
```
- Menu Fluent **sempre sobreposto** à surface nativa (nunca escondido)
- Ativação **personalizável** pelo usuário, com suporte a **combinação de teclas** (janela hold+press) — hotkey sempre processado independente do estado atual, com prioridade sobre input de core
- **Comportamento de áudio/pause**: ao entrar em `MenuFocused`, o core **pausa** (emulação + stream de áudio); resume ao voltar pra `GameFocused`

### ToastLayer (camada independente, fora da state machine)
- Nunca captura input, nunca pausa o core, sempre renderiza por cima de `GameFocused` ou `MenuFocused`
- Fila de toasts (não substituição), origem tanto do backend (eventos Tauri) quanto do frontend
- Erros bloqueantes **não** são modelados como toast — exigem transição forçada pra `MenuFocused` com tela dedicada

---

## 4. Filtros de Imagem e Decoração

- `ShaderChain`: cadeia de passes encadeáveis, com suporte ao formato **slang** (Mega Bezel/RetroArch)
- `DecorationResolver`: resolução em cascata **rom → sistema → default**, inspirado no RetroBat (pastas de override do usuário + pastas padrão)
- **Mega Bezel e decoração externa são mutuamente exclusivos por padrão** (preset com `includes_bezel=true` pula a resolução de decoração), com toggle avançado pra empilhar manualmente
- **ReShade FX**: viável tecnicamente (licença BSD 3-clause, compilador standalone), mas **backlog** — a maior parte do valor diferencial dele (efeitos dependentes de depth buffer) não se aplica à maioria dos cores 2D

### Schemas
```sql
shader_presets (id, name, source_path, format, is_builtin, includes_bezel)
shader_chain_assignments (id, scope['default'|'system'|'rom'], system_id, rom_id, preset_id)
shader_parameter_overrides (id, assignment_id, parameter_key, value)

decoration_packs (id, name, source, base_path)
decoration_assignments (id, scope, system_id, rom_id, pack_id, asset_path)
```

---

## 5. Cores Libretro

- **Catálogo/listagem**: leitura em tempo real do `buildbot.libretro.com` (sem índice próprio espelhado)
- **Curadoria de status experimental**: arquivo estático versionado no repo do app, atualizado manualmente por release
- **Distribuição no Android**: app **não será distribuído na Google Play** → `targetSdkVersion` baixo (ex: 28) para evitar a restrição de `dlopen` do Android 10+ → **download dinâmico igual ao desktop**, sem catálogo restrito. Documentado como dívida técnica a monitorar (futuras versões do Android podem apertar o piso mínimo aceito)

### `CoreOptionsStore`
```sql
core_options_schema (id, core_id, option_key, display_name, option_type, choices, default_value)
core_options_values (id, core_id, option_key, value)
```
Populado em runtime via `retro_core_options`/`retro_core_options_v2` do próprio core — sem tela custom por core.

---

## 6. Input

- **Reconhecimento automático (desktop)**: `gilrs` + SDL_GameControllerDB, com camada de tradução SDL-layout → RetroPad
- **Reconhecimento automático (Android)**: cobertura limitada a **Xbox/PlayStation via Bluetooth** no MVP; resto cai no fluxo manual. Banco próprio mais amplo fica como backlog
- **Hotkeys de sistema**: camada separada de `input_mappings`, com prioridade sobre ações de jogo
- **UI de captura de binding** (componente único, comportamento parametrizado por `target`):
  - Hotkeys de sistema → suporta combinação (janela hold+press)
  - Mapeamento de controle → captura de tecla única

### Schema
```sql
controller_mappings (guid, display_name, layout_json, source)
device_port_assignment (guid, port_index)
system_hotkeys (ação, combinação de teclas/botões, dispositivo)
```
Sem `user_profile_id` — configuração única por instalação.

---

## 7. Áudio

- **Dynamic Rate Control** (não resample fixo) para sincronia áudio/vídeo — margem de ajuste configurável
- **Identificação de dispositivo**: `cpal` (desktop) / Oboe (Android), por ID persistente do SO, não índice
- **Fallback**: se o dispositivo salvo não for encontrado na inicialização, usa o padrão do sistema + toast informativo (via `ToastLayer`)

### Schema
```sql
audio_config (id, output_device_id, output_device_name, rate_control_enabled,
              rate_control_delta, sample_rate_preference)
```

---

## 8. Save States e Save RAM

- **Timing**: save imediato entre frames (nunca no meio de um `retro_run`), sem pausar o core — thumbnail capturado no mesmo instante do serialize
- **Armazenamento**: arquivo em disco (não BLOB no SQLite); banco guarda só metadata
- `save_states` e `save_ram` são entidades distintas (semânticas diferentes no libretro: `retro_serialize` vs `retro_get_memory_data`)
- `core_id` obrigatório no save state — states não são portáveis entre cores diferentes

### Schema
```sql
save_states (id, rom_id, core_id, slot, file_path, thumbnail_path, created_at, play_time_at_save)
save_ram (id, rom_id, core_id, file_path, updated_at)
```

---

## 9. Biblioteca e Metadata (Scraping)

- **Matching por hash** (CRC32/MD5), não por nome de arquivo
- **Match automático**: só com **hash exato**; qualquer busca heurística/por nome vai sempre pra revisão manual (critério objetivo, não score de confiança de terceiros)
- **Provedor de metadata**: configurável pelo usuário (IGDB, ScreenScraper, TheGamesDB, etc.), não fixo — pode ter múltiplos ativos com cascata

### Schema
```sql
roms (id, file_path, crc32, md5, system_id, added_at)
scrape_matches (id, rom_id, provider, external_id, confidence_score, status)
game_metadata (id, rom_id, title, description, cover_url, release_date, genre, provider_source)
```

---

## 10. Estado de Cliente (Frontend)

- **Zustand** para estado de UI/cliente decidido fora do React (ex: `InputFocus` espelhado do backend Rust via eventos Tauri) — stores pequenas e específicas, não uma store global espelhando o banco
- Dados que vivem no SQLite passam por IPC + cache de query (ex: TanStack Query), nunca duplicados em Zustand

---

## Itens em Backlog (fora do MVP, com encaixe arquitetural já garantido)

| Item | Motivo de adiar |
|---|---|
| Suporte a ReShade FX no `ShaderChain` | Baixo valor diferencial pro caso de uso (maioria dos efeitos dependem de depth buffer, indisponível na maioria dos cores) |
| Vulkan HW render por-core | Maior risco técnico; maioria dos cores do dia 1 não precisa |
| Banco próprio de mapeamento de controle Android (além de Xbox/PS) | Evita manutenção manual contínua sem base de usuários ainda |

---

## Próximos Passos Sugeridos

1. Estrutura inicial do repositório (monorepo: Cargo workspace + pnpm workspace)
2. Implementação do crate `domain` (traits/portas puras, sem I/O) como base — tudo mais depende dele estar estável
3. Migrations iniciais do SQLite com os schemas acima
4. Adapter `core-loader-desktop` (caminho mais maduro) antes do mobile
