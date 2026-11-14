# Plano: robustez da GPU, `wgpu` × `windows` e extração de crates

Planejado em 2026-11-13, a partir da análise do projeto de 2026-11-12 (itens 9 e 12) e do
bloqueio das atualizações do Tauri. **Nada aqui foi executado.** Cada etapa só começa com o
aviso do dono do projeto, e as etapas A e B tocam GPU: toda decisão sobre `wgpu`, Vulkan,
DX12 ou OpenGL cita a documentação oficial no commit (regra do `CLAUDE.md`), e o que não for
verificado fica marcado como tal.

Ordem sugerida: A1 → B1 → (decisão) → C → A2. A1 é quase sem risco e melhora o diagnóstico;
B1 é só uma experiência descartável; C não toca GPU; A2 fica para quando houver relato real.

## Etapa A — tirar os pânicos do caminho de vídeo (item 9)

Hoje: 20 `unwrap` em código de GPU fora de testes. Um pânico na thread de vídeo derruba a
imagem, enquanto core e áudio seguem (2026-10-29, "jogo sem imagem com o som tocando").

### A1. Travas de `Mutex` em `core-loader-desktop/src/vk_frame.rs` — risco baixo

14 dos 15 `unwrap` do arquivo são `lock().unwrap()`. Só entram em pânico se outra thread já
caiu segurando a trava; transformam uma falha em duas.

Tarefas:
1. Trocar por `lock().unwrap_or_else(|p| p.into_inner())`, o padrão já usado em
   `emu-session/src/session.rs`. Não muda a ordem das chamadas Vulkan nem a sincronização.
2. O `h.join().unwrap()` da linha ~565 é de teste: confirmar e deixar.
3. Conferir: `cargo clippy --workspace --all-targets -D warnings`; os testes de GPU
   ignorados desta máquina (`win32_interop_*`, `vk_context`, `from_adopted_vulkan_runs_the_chain`,
   `vk_interop_id_reads_this_device`), como em 2026-11-06.
4. Registrar no histórico; commit separado.

Pronto quando: zero `lock().unwrap()` em `vk_frame.rs`, testes de GPU locais passando.

### A2. Invariantes em `apps/desktop/src-tauri/src/gpu/surface.rs` — risco médio

5 `unwrap` afirmam estado: `self.surface.as_ref().unwrap()` (linhas ~341, ~404, ~509),
`self.comp.target…unwrap()` (~406) e `self.passes.last()…unwrap()` (~410).

Por que é médio: virar `Result` muda o fluxo do render. Um quadro antes da surface existir
passaria de pânico para "pular o quadro" ou "voltar ao shader simples"; uma escolha errada
vira tela preta silenciosa, pior de diagnosticar que o pânico.

Tarefas:
1. Para cada ponto, descobrir no código quando o `None` pode acontecer (ordem de criação da
   surface, troca de preset, perda do device) e escrever isso no comentário.
2. Um ponto por commit: `let Some(..) else { log::error!(..); return; }` ou o caminho de
   recuperação que já existe (`recover_from_gpu_error`, 2026-10-29).
3. Teste de GPU ignorado novo para cada ponto que der para provocar (ex.: renderizar antes de
   configurar a surface).
4. Conferir nas mesmas GPUs; marcar como não verificado em Linux, AMD e Intel (B6b do TASKS).

Pronto quando: os 5 pontos sem `unwrap`, cada um com o motivo do `None` escrito e o
comportamento escolhido testado ou marcado como não verificado.

Sugestão: só fazer A2 depois de um relato real de pânico num desses pontos.

## Etapa B — `windows` 0.61 × `wgpu-hal` 30 (desbloqueia o Tauri)

Situação (conferida em 2026-11-13): qualquer `cargo update` em `tauri-plugin-updater` 2.14.1,
`wry` 0.57.2 ou `tauri-plugin-log` 2.10.2 faz o resolvedor pôr a crate `windows` 0.61 em
`gio`/`gtk`, e o `wgpu-hal` 30.0.1 deixa de compilar no Windows
(`dx12/suballocation.rs`, tipos do `gpu-allocator` 0.28). Enquanto isso, a `zip` 4.6.1
vulnerável (RUSTSEC-2026-0367/0368/0369/0429) segue no lock só por causa do updater 2.14.0.

### B1. Experiência com o `wgpu` 30.0.2 — sem commit

1. Copiar o `Cargo.lock`; `cargo update -p wgpu -p wgpu-hal -p wgpu-core`; depois
   `cargo update -p tauri-plugin-updater`.
2. `cargo check --workspace --all-targets` nesta máquina (Windows) e `scripts/check-windows.sh`.
3. Se compilar: testes de GPU ignorados + jogo real com shader, moldura e HDR (vídeo nativo).
4. Restaurar o lock de qualquer jeito; anotar o resultado no TASKS.

Documentação a consultar antes de concluir: changelog do `wgpu` 30.0.2 (docs.rs/crates.io),
`gpu-allocator` (dependência do backend DX12), docs.rs da crate `windows`.

### B2. Se o 30.0.2 resolver — risco baixo

Commit com o lock novo, updater 2.14.1, `wry` e `plugin-log` atualizados, `zip` 4.6.1 fora
do lock e os quatro RUSTSEC tirados do `ignore` do `ci.yml`. CI nas três plataformas.

### B3. Se não resolver — escolher uma saída (decisão do dono do projeto)

| Saída | Risco | Custo |
|---|---|---|
| Esperar `wgpu`/`gpu-allocator` compatível | nenhum agora; `zip` 4.6.1 continua | zero; reconferir a cada mês |
| Fixar a `windows` com `[patch]` ou versão exata | médio: Tauri e `wgpu` com expectativas diferentes | baixo, mas frágil a cada update |
| Subir o `wgpu` para o próximo major | alto: API de `gpu/mod.rs` (1.337 linhas), `surface.rs`, `vk_blit.rs`, `video-surface`; shaders, HDR, interop Vulkan/GL | alto; etapa própria com o mesmo cuidado da Fase 8 |

Recomendação: esperar, reconferindo com o B1 a cada mês. O risco da `zip` 4.6.1 é baixo
(o plugin confere a assinatura antes de abrir o arquivo).

## Etapa C — Fase 6: catálogo, atualização de cores e scraping em crates (item 12)

Não toca GPU. Risco baixo em execução (o comportamento não muda) e médio no build: três
lugares compilam o workspace (Linux, Windows nativo e o cross-check), e a fumaça do catálogo
depende do `core_catalog`.

Acoplamento atual (`apps/desktop/src-tauri/src`):

| Módulo | Linhas | Depende de |
|---|---|---|
| `http.rs` | 57 | — |
| `repo_version.rs` | 205 | `http` |
| `core_catalog.rs` | 1.211 | `http`, `app_error`; nos testes, `core_updates` e `smoke_roms` |
| `core_updates.rs` | 378 | `core_catalog`, `repo_version`, `app_error` |
| `smoke_roms.rs` | 384 | — (só testes da fumaça) |
| `scraping.rs` | 1.098 | `credentials`, `covers`, `http`, banco |

Tarefas, uma crate por commit e com `cargo check --workspace --all-targets` + testes a cada passo:
1. **`crates/net`**: `http.rs` (cliente com user-agent e timeouts) e `repo_version.rs`.
2. **`crates/core-catalog`**: `core_catalog.rs`, `core_updates.rs` e `smoke_roms.rs`. O
   `AppError` fica no app: a crate devolve um erro próprio com `kind`
   (rede, disco, não encontrado) e o app converte em `From`. A fumaça (`catalog_smoke`) vai
   junto; o `catalog-smoke.yml` passa a rodar `cargo test -p core-catalog`.
3. **`crates/scraping`**: depois de separar o que depende do Tauri (`covers`, chaveiro em
   `credentials`) por trait, como o `OsKeyring` já faz.
4. Atualizar `CONTRIBUTING.md`, o mapa de crates do `README` e os workflows.

Pronto quando: `src-tauri` sem esses módulos, CI verde nas três plataformas, fumaça semanal
verde, tempo de `cargo test -p reemu-desktop` medido antes e depois.

## Fora deste plano

- Espaços numerados de save state (Slot 1, 2…): backend pronto, falta a interface.
- Majors adiados do Dependabot: `sqlx` 0.9, TypeScript 7, Vite 8 + `plugin-react` 6.
- Stella travando na fumaça do Windows (causa em aberto).
