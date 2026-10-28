# 16 — Conquistas (RetroAchievements)

**Estado:** planejado (pesquisa de 2026-10-28). Nenhum código ainda.
**Depende de:** 01 (core libretro), 06/07 (sessão e processo do core),
save states, Configurações e a página do jogo.

## O que é

O RetroAchievements (RA) é a plataforma aberta de conquistas para jogos
retrô. O jogador faz login com a conta do RA, e o emulador avalia, a cada
quadro, condições sobre a memória do jogo, como "a vida chegou a zero" ou "o
chefe da fase 3 caiu". Quando uma condição fecha, a conquista é enviada ao
servidor. Também há placares (leaderboards), "rich presence" (o que o jogador
está fazendo agora, mostrado no perfil) e o modo **hardcore**, que vale mais
pontos e proíbe ajudas.

## Fontes oficiais

- rcheevos, a biblioteca oficial de integração: <https://github.com/RetroAchievements/rcheevos>
  - licença **MIT**, compatível com a MIT do ReEmu;
  - última release **v12.5.0**, de 2026-09-14. A recomendação é usar a
    `master`, que é a última release oficial.
- Integração pelo `rc_client`: <https://github.com/RetroAchievements/rcheevos/wiki/rc_client-integration>
- Identificação de jogos (hash por sistema): <https://docs.retroachievements.org/developer-docs/game-identification.html>
- Emuladores e cores sem suporte: <https://docs.retroachievements.org/developer-docs/unsupported-emulators-and-cores.html>
- `src/libretro/rc_libretro.h`, ajudas para frontends libretro: regiões de
  memória, opções de core proibidas no hardcore e hash de jogos com vários
  discos.

## O que a documentação exige (citado)

- **Rede por nossa conta:** "rcheevos does *not* provide HTTP network
  connections". O ReEmu faz o HTTPS e devolve a resposta ao rcheevos.
  "callback must be called with callback_data, regardless of the outcome of
  the HTTP call."
- **User-Agent:** formato `<product>/<product-version> (<system-information>)
  <extensions>`, com nome único sem espaços e versão numérica crescente.
  "the server will turn hardcore unlock requests into softcore unlock
  requests" se o User-Agent faltar ou não for reconhecido. **É preciso falar
  com a equipe do RA (RAdmin) para validar o hardcore do ReEmu.**
- **Login:** `rc_client_begin_login_with_password` na primeira vez. Depois,
  guardar o **token** e usar `rc_client_begin_login_with_token`; a senha não
  fica guardada.
- **Jogo:** `rc_client_begin_identify_and_load_game`, com o console (ou
  `RC_CONSOLE_UNKNOWN`) e o caminho ou o buffer da ROM. Ao carregar, mostrar
  uma "placa" do jogo, para o jogador saber que as conquistas estão ativas.
- **Por quadro:** "rc_client_do_frame should be called for every frame,
  whether or not the emulator actually displays them".
- **Pausa:** sem `do_frame`; `rc_client_idle` cerca de 1 vez por segundo. No
  hardcore, perguntar `rc_client_can_pause` antes de pausar, para evitar que o
  jogador pause repetidamente.
- **Reset:** "When the emulator is reset, the runtime should be as well"
  (`rc_client_reset`).
- **Save states:** "Whenever a save state is created, the runtime state should
  be included" (`rc_client_progress_size` e
  `rc_client_serialize_progress`). Ao carregar, usar
  `rc_client_deserialize_progress`.
- **Hardcore:** começa ligado. Trocar com `rc_client_set_hardcore_enabled`
  dispara um `RESET`, e o emulador reinicia o jogo.
  - **Proibido:** carregar save state, cheats que mudam o jogo, rewind, câmera
    lenta e avanço quadro a quadro, depurador (sobretudo visualizar memória),
    reproduzir entradas gravadas.
  - **Permitido:** salvar estado, upscaling, avançar rápido, gravar entradas.
- **Eventos** (`rc_client_set_event_handler`):
  - conquista desbloqueada;
  - indicador de desafio e de progresso;
  - placar iniciado, falho ou enviado, e o rastreador do placar;
  - jogo e subconjunto completos;
  - `RESET` e `SERVER_ERROR`.
- **Fila:** o texto do rastreador "may be updated in a non-thread safe manner
  within rc_client_do_frame". A interface usa uma cópia.

## Onde isso encaixa no ReEmu

- **Processo do core:** o core roda no `reemu-core-host` (`core-host-desktop`),
  falando com o app pelo `core-ipc`. O `rc_client_do_frame` lê a memória do
  jogo muitas vezes por quadro, então **o `rc_client` vive no processo do
  core**. Ler a memória pelo IPC a cada quadro custaria caro demais.
  - Exceção: o core Vulkan que roda no processo do app (§Beetle,
    `step_vk_local`). Por isso a integração fica num crate próprio, usado pelos
    dois processos.
- **Memória:** o ReEmu ainda **não trata** o
  `RETRO_ENVIRONMENT_SET_MEMORY_MAPS` (36): o log mostra "environment 36
  (0x10024) não tratado". Muitos sistemas precisam do mapa, e com ele o
  `rc_libretro_memory_init` monta as regiões. O `retro_get_memory_data` já é
  resolvido (`core-loader-desktop/src/raw.rs`).
- **Rede:** um único lugar faz HTTPS, o app (`reqwest`), com o User-Agent do
  ReEmu. O processo do core manda o pedido pelo IPC e recebe a resposta. Assim
  o token e o User-Agent ficam num lugar só.
- **Hash:**
  - o `archive.rs` já extrai ROMs `.zip`/`.7z` para um arquivo temporário, e o
    `rc_hash` usa esse caminho;
  - **fliperama** usa o nome do arquivo original (ex.: `galaga.zip` vira
    `galaga`), então é preciso passar o caminho original;
  - **CHD** precisa de um leitor próprio no `rc_hash` (libchdr, BSD-3);
  - **jogos com vários discos** (`.m3u`) usam o `rc_libretro_hash_set_*`.
- **O que o hardcore bloqueia no ReEmu:** hoje não há rewind, câmera lenta,
  cheats nem avanço quadro a quadro. Fica:
  - **carregar save state** (salvar continua livre);
  - as **opções de core proibidas**
    (`rc_libretro_get_disallowed_settings`) e os pares core/sistema não
    permitidos (`rc_libretro_is_system_allowed`);
  - o **limite de pausas** (`rc_client_can_pause`).
- **Avisos durante o jogo:** no Windows o vídeo é uma janela nativa **por
  cima** do WebView2, então o `ToastLayer` da interface fica escondido
  enquanto se joga. Os avisos de conquista precisam ser desenhados **na
  própria imagem do jogo**, com o wgpu. A decisão exata (biblioteca de texto
  para wgpu, como o `glyphon`) fica para a fase 4, consultando a documentação
  oficial do wgpu, como manda o `CLAUDE.md`.
- **Segredo:** o token do RA vai para o cofre do sistema (Gerenciador de
  Credenciais do Windows, Secret Service no Linux), não para o banco nem para
  o `app-settings.json`.

## Plano de ação

Cada fase termina com algo testável e que pode ir para uma release.

### Fase 0 — Preparo (sem código)
- [ ] **Dono do projeto:** falar com a equipe do RA para registrar o
      User-Agent `ReEmu/<versão>` e combinar o que eles pedem para validar o
      hardcore. Até lá, as conquistas funcionam só em **softcore**.
- [ ] Ler as regras de uso do RA para clientes (termos e uso da API) e
      registrar aqui.
- [ ] Escolher o caminho do código C: o crate `rcheevos-sys` 0.1.0 está
      parado desde 2024-11. **Proposta:** trazer o fonte do rcheevos na tag
      v12.5.0 para um crate nosso e compilar com `cc`, com as declarações FFI
      escritas à mão só para o que usamos, sem `bindgen` (que exige libclang
      na máquina e no CI).

### Fase 1 — Base (memória + biblioteca)
- [ ] Crate `crates/achievements` com o rcheevos compilado (cc) e a FFI do
      `rc_client`, `rc_hash` e `rc_libretro`, com testes de fumaça.
- [ ] `core-loader-desktop`: tratar o `SET_MEMORY_MAPS` (guardar os
      descritores) e expor o mapa e o `get_memory_data`/`get_memory_size`.
- [ ] Teste: `rc_libretro_memory_init` monta as regiões num core real
      (Snes9x, por exemplo) e a leitura bate com a RAM do core.

### Fase 2 — Conta
- [ ] Configurações › Conquistas:
  - entrar com usuário e senha do RA, guardando só o token no cofre do
    sistema;
  - sair;
  - interruptor do hardcore;
  - textos em pt-BR, en e es.
- [ ] O app faz o HTTPS do RA (`reqwest`) com o User-Agent do ReEmu.
- [ ] O menu do avatar, "Minhas conquistas", deixa de ficar desabilitado
      quando há conta.

### Fase 3 — Conquistas no jogo (softcore)
- [ ] No processo do core:
  - criar o `rc_client`;
  - carregar o jogo com o hash certo (arquivo extraído, nome original no
    fliperama);
  - `do_frame` a cada quadro, `idle` na pausa e `reset` no reset.
- [ ] Pedidos HTTP do `rc_client` e eventos (conquista, placa do jogo, erro)
      vão ao app pelo `core-ipc`.
- [ ] Save state com o progresso das conquistas (`serialize_progress`), ao
      salvar e ao carregar.

### Fase 4 — Avisos e telas
- [ ] Avisos durante o jogo desenhados no vídeo (wgpu): placa do jogo ao
      abrir, conquista desbloqueada com o emblema, e o som.
- [ ] Página do jogo: aba Conquistas, com emblema, nome, descrição, pontos e
      o que já foi desbloqueado. O mesmo no menu de pausa.
- [ ] Emblemas vêm do servidor de mídia do RA, com cache local como as capas.

### Fase 5 — Hardcore
- [ ] Bloquear carregar save state no hardcore, avisando o jogador.
- [ ] Respeitar as opções de core proibidas e os sistemas não permitidos
      (`rc_libretro_*`).
- [ ] Limitar a pausa com o `rc_client_can_pause`.
- [ ] Ao ligar o hardcore no meio do jogo: reiniciar o jogo, porque o
      `rc_client` dispara um `RESET`.

### Fase 6 — O resto do RA
- [ ] Placares: rastreador na tela, aviso de envio e de falha.
- [ ] Indicadores de desafio e de progresso.
- [ ] Rich presence: o `rc_client` envia sozinho, e o app só mantém a sessão.
- [ ] CHD (leitor no `rc_hash`) e jogos com vários discos (`m3u`).
- [ ] Na biblioteca, mostrar quais jogos têm conquistas
      (`rc_client_begin_fetch_hash_library` ou equivalente; conferir na doc
      quando chegar aqui).

### Fase 7 — Validação com o RA
- [ ] Testar os sistemas e cores que o ReEmu usa contra a lista de
      emuladores e cores sem suporte.
- [ ] Mandar o ReEmu para a validação do hardcore com o User-Agent
      registrado na fase 0.

## Riscos e pontos em aberto

- **Aprovação do hardcore:** depende da equipe do RA. Sem ela, só softcore,
  sem prejuízo para as fases 1 a 4.
- **Cores e sistemas:** alguns cores libretro não são aceitos pelo RA, ou só
  são aceitos com certas opções. O `rc_libretro` traz essas listas.
- **Avisos sobre o vídeo nativo:** desenhar texto no wgpu é trabalho novo de
  GPU; a regra do `CLAUDE.md` vale (documentação oficial em cada decisão).
- **Core Vulkan no processo do app** (§Beetle): a integração também precisa
  funcionar lá. Pouco usado; pode vir depois.
