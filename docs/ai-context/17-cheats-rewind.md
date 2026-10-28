# 17 — Cheats e rewind

**Estado:** planejado (pesquisa de 2026-10-28). Nenhum código ainda.
**Depende de:** 01 (core libretro), 06/07 (sessão e processo do core), save
states, atalhos (`input_desktop::hotkeys`) e, para cheats de memória, o mapa de
memória da fase 1 das conquistas (`16-retroachievements.md`).
**Conflito conhecido:** no modo hardcore do RetroAchievements, cheats e rewind
são **proibidos**. Ver a fase 5 do doc 16.

## Fontes oficiais

- `libretro.h` (libretro-common, master):
  - `retro_cheat_reset()`: "Reset all the active cheats to their default
    disabled state";
  - `retro_cheat_set(unsigned index, bool enabled, const char *code)`:
    "Enable or disable a cheat";
  - `retro_serialize_size` / `retro_serialize` / `retro_unserialize`;
  - `RETRO_ENVIRONMENT_GET_SAVESTATE_CONTEXT` (72 | experimental) com
    `RETRO_SAVESTATE_CONTEXT_RUNAHEAD_SAME_INSTANCE`: "guaranteed to be loaded
    within the same session, address space, and binary … This context
    generally implies the use of runahead or rewinding … Savestate code that
    runs in this context should be fast";
  - `RETRO_ENVIRONMENT_SET_SERIALIZATION_QUIRKS` (44), com
    `RETRO_SERIALIZATION_QUIRK_*`:
    - `INCOMPLETE`: "should not be relied upon for frame-sensitive frontend
      features";
    - `MUST_INITIALIZE`: as funções de serialização "will initially fail";
    - `CORE_VARIABLE_SIZE` / `FRONT_VARIABLE_SIZE`: o tamanho do estado pode
      mudar na sessão;
    - `SINGLE_SESSION`: "Serialized state can only be loaded during the same
      session";
    - `ENDIAN_DEPENDENT` / `PLATFORM_DEPENDENT`.
- Banco de cheats do libretro: <https://github.com/libretro/libretro-database>,
  pasta `cht/<Sistema>/<Jogo>.cht`. Licença **CC-BY-SA 4.0**: baixar sob
  demanda e dar o crédito, sem embutir no app (MIT).
  - Formato, visto num arquivo real:
    ```
    cheats = 12
    cheat0_desc = "P1/Team 1 Has Lots of Money"
    cheat0_code = "7F80CAFF+7F80CCFF+7F80CBFF"
    cheat0_enable = false
    ```
- **Não copiar código do RetroArch** (GPL): o gerenciador de cheats e o
  `state_manager` do rewind dele. Parser e rewind são escritos do zero,
  seguindo só o `libretro.h` e o formato dos dados.

## O ReEmu hoje

- **Cheats:** nada. O `core-loader-desktop` nem resolve o `retro_cheat_reset`
  e o `retro_cheat_set` (`raw.rs`).
- **Save state:** existe. O `retro_serialize`/`retro_unserialize` roda no
  `reemu-core-host`, e o estado vai e volta pelo `core-ipc`
  (`ToChild::SaveState` / `RestoreState`, `emu-session`).
- **Quirks:** o ReEmu não trata o `SET_SERIALIZATION_QUIRKS` nem o
  `GET_SAVESTATE_CONTEXT`. Hoje um core com `SINGLE_SESSION` grava save state
  em disco que não vai carregar depois, um defeito que já existe, à parte
  deste plano.
- **Avanço rápido e câmera lenta:** não existem.

## Cheats

**Como funciona:** o frontend entrega ao core o código do cheat como texto.
Game Genie, Action Replay, GameShark, CWCheat e outros, conforme o core:
- quem entende o código é o core;
- o suporte **varia de core para core**, e um core pode ignorar sem avisar;
- `index` é a posição do cheat na lista do frontend;
- ao trocar a lista, `retro_cheat_reset` e depois `retro_cheat_set` para cada
  cheat ligado.

**Cheats de memória (segunda etapa):** para cores sem suporte, ou para cheats
que não são de código, o frontend escreve na RAM do jogo a cada quadro
(endereço e valor). Para isso precisa do mapa de memória
(`SET_MEMORY_MAPS` + `retro_get_memory_data`), o mesmo da fase 1 das
conquistas. Uma busca na memória ("encontrar vidas") pode vir depois.

**Onde roda:** no processo do core, como o save state. O app manda a lista de
cheats pelo IPC, e o processo do core aplica no core ao carregar e a cada
mudança.

## Rewind

**Como funciona:**
- a cada N quadros, o processo do core guarda um `retro_serialize` num
  buffer circular na memória;
- segurando o botão de rewind, ele carrega os estados de trás para frente,
  com `retro_unserialize` e um quadro rodado para gerar a imagem;
- o áudio fica mudo durante o rewind.

**Custo:** o estado inteiro de cada sistema pesa muito diferente. SNES e
Mega Drive ficam em centenas de KB; N64, PS1, PSP e Dreamcast chegam a vários
MB, ou dezenas. **Medir antes de decidir**, com os testes que já carregam
cores reais (`REEMU_TEST_GL_CORE`).
- **Delta:** cada estado guarda só a diferença (XOR) para o anterior,
  comprimida (`zstd` ou `lz4_flex`, MIT/BSD). Escrito do zero.
- **Limite de memória** do buffer (ex.: 256 MB) e intervalo de captura por
  sistema (1 quadro nos leves; mais espaçado ou desligado por padrão nos
  pesados).
- **Contexto:** responder `GET_SAVESTATE_CONTEXT` com `RUNAHEAD_SAME_INSTANCE`
  nas capturas do rewind, para o core poder usar o caminho rápido, e
  `NORMAL` nos save states em disco.
- **Quirks:** rewind desligado, com aviso, para core com `INCOMPLETE` ou que
  ainda não inicializou (`MUST_INITIALIZE`). Tamanho variável tratado
  (`FRONT_VARIABLE_SIZE`).
- **Cores com GPU** (GL e Vulkan, como PPSSPP e flycast): conferir se o
  estado inclui o que a imagem precisa; testar core a core.

**Onde roda:** inteiro no processo do core, sem passar estados pelo IPC. O app
só manda "começar/parar rewind".

## Plano de ação

### Fase 0 — Base comum
- [ ] Tratar `SET_SERIALIZATION_QUIRKS` e `GET_SAVESTATE_CONTEXT` no
      `core-loader-desktop`, e guardar os quirks do core.
- [ ] Corrigir o defeito atual: não gravar em disco o save state de core com
      `SINGLE_SESSION`, avisando o jogador.
- [ ] Medir `retro_serialize_size` e o tempo de `retro_serialize` nos cores
      da biblioteca (snes9x, genesis_plus_gx, mGBA, pcsx_rearmed ou beetle,
      parallel_n64, PPSSPP, flycast). Registrar no doc.

### Fase 1 — Cheats por código
- [ ] Resolver `retro_cheat_reset` e `retro_cheat_set` no core-loader e
      expor pelo IPC (`ToChild::SetCheats(Vec<Cheat>)`).
- [ ] Banco: tabela de cheats por ROM (descrição, código, ligado, origem:
      usuário ou banco), aplicados ao carregar o jogo.
- [ ] Menu de pausa › Cheats: lista com interruptores, adicionar código à
      mão (com o teclado na tela pelo controle), editar e apagar. Textos em
      pt-BR, en e es.
- [ ] Aviso quando o core não suporta cheats por código (descobrir como
      detectar; se não houver jeito, deixar claro que depende do core).

### Fase 2 — Banco de cheats do libretro
- [ ] Baixar o `.cht` do jogo sob demanda do `libretro-database`, pelo
      sistema e pelo nome no padrão No-Intro, como as molduras. Mostrar o
      crédito (CC-BY-SA 4.0) na tela e em Informações e licenças.
- [ ] Parser próprio do `.cht` (`cheats`, `cheatN_desc`, `cheatN_code`,
      `cheatN_enable`; campos de memória mais novos, se houver), com testes.
- [ ] Importar um `.cht` do disco.

### Fase 3 — Rewind
- [ ] Buffer circular com delta comprimido no processo do core, com limite
      de memória e intervalo por sistema.
- [ ] Atalho de segurar: no teclado e no controle (combinação, pelo
      `ComboHotkeyResolver`). Áudio mudo e indicador na tela durante o
      rewind.
- [ ] Configurações › Sistema (ou Jogo): ligar/desligar, memória máxima e
      intervalo. Desligado por padrão nos sistemas pesados, conforme a
      medição da fase 0.
- [ ] Testes: rewind em core real volta a um estado igual ao gravado (hash da
      RAM), e o buffer respeita o limite.

### Fase 4 — Cheats de memória
- [ ] Com o mapa de memória (fase 1 das conquistas), aplicar cheats de
      endereço e valor a cada quadro no processo do core.
- [ ] Busca na memória (opcional): filtrar endereços por valor igual, maior
      ou menor entre quadros.

### Fase 5 — Conquistas (hardcore)
- [ ] Quando o hardcore do RetroAchievements estiver ligado: cheats e rewind
      desligados e escondidos, e ligar um deles pede para sair do hardcore
      (`rc_client_set_hardcore_enabled(false)`). Ver o doc 16, fase 5.

## Riscos e pontos em aberto

- **Suporte a cheats por core:** não há, no `libretro.h`, um jeito de o core
  dizer se aceita `retro_cheat_set`. Pode ser preciso uma lista por core,
  testada.
- **Rewind nos sistemas pesados:** depende da medição; pode ficar desligado
  por padrão ou só com intervalo maior.
- **Vídeo nativo:** o indicador de rewind na tela tem a mesma limitação dos
  avisos durante o jogo (backlog "Menus e avisos por cima do jogo em
  movimento" do `TASKS.md`).
- **Licença do banco de cheats:** CC-BY-SA 4.0 exige crédito e o
  compartilhamento das mudanças nos dados. Baixar sob demanda, sem alterar os
  arquivos, evita misturar com o código MIT.
