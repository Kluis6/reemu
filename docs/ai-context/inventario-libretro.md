# Inventário de compatibilidade libretro por core

**Recorte dos índices** — 2026-11-18, reconferido em 2026-11-24. Catálogo atualizado em 2026-11-28.
A disponibilidade indica presença no índice de pacotes. A compatibilidade
será comprovada pelo [roteiro de implementação](26-compatibilidade-libretro.md).

- **356 IDs** na união dos metadados de cores e dos dois índices.
- **261 IDs com pacote desktop**, dos quais 152 já estão no catálogo e 109 faltam. Entre os faltantes, 106 aparecem nos dois índices e 3 apenas no de Linux x64.
- **95 IDs adicionais** dos metadados oficiais ficam na fila de investigação de builds e alvos.

## Fontes do recorte

- Catálogo local na data do recorte. [Código do ReEmu](../../crates/core-catalog/src/catalog.rs)
- 349 arquivos de metadados de cores na revisão `eac3a2784d42`, excluídos o modelo `00_example_libretro.info` e o arquivo de avisos `open-source-notices.info`. [Revisão do Core Info](https://github.com/libretro/libretro-core-info/tree/eac3a2784d428444aa2bd472039b5c5540141af1)
- Windows x64, 258 IDs. SHA-256 do índice `82a8986115b431f154685c30f64ebe91de902dba66bd29f5e525d2ac0baf5dab`. [Índice do buildbot](https://buildbot.libretro.com/nightly/windows/x86_64/latest/.index-extended)
- Linux x64, 261 IDs. SHA-256 do índice `d913ba47e22bf5f21956a7892740e8380c6a5942b3428cb269fb86edfa0111b7`. [Índice do buildbot](https://buildbot.libretro.com/nightly/linux/x86_64/latest/.index-extended)

## IDs publicados para desktop fora do catálogo (109)

Os exemplos de extensão vêm dos arquivos oficiais de informações.
Confirme o contrato do binário escolhido. Cada entrada passa pelos critérios
do roteiro antes de ser publicada como compatível.

### Pilotos e casos específicos (12)

- [ ] `azahar_libretro` — 3DS. Criar a integração 3DS e validar duas telas, toque e armazenamento. **Pacote:** Windows e Linux x64.
- [ ] `cemu_libretro` — Wii U. Validar WUA/RPX, GamePad e persistência de `mlc01` no fluxo Wii U. **Pacote:** Windows e Linux x64.
- [ ] `citra2018_libretro` — 3DS. Validar boot, layout, toque e saves da revisão 2018 como variante própria. **Pacote:** Windows e Linux x64.
- [ ] `citra_libretro` — 3DS. Validar as opções e o toque da DLL Citra após integrar a plataforma 3DS. **Pacote:** Windows e Linux x64.
- [ ] `panda3ds_libretro` — 3DS. Confirmar contexto gráfico do binário e testar layout e toque com a plataforma 3DS. **Pacote:** Windows e Linux x64.
- [ ] `parallel_dc_libretro` — Sega Dreamcast. Validar a variante Dreamcast separadamente do Flycast e registrar os backends selecionáveis. **Pacote:** Windows e Linux x64.
- [ ] `pcee2_libretro` — Sony PlayStation 2. Validar como alternativa de PS2 e conferir a negociação gráfica e os caminhos de firmware e saves. **Pacote:** Windows e Linux x64.
- [ ] `play_libretro` — Sony PlayStation 2. Testar Play! como alternativa para PS2 e confirmar o renderizador anunciado no binário. **Pacote:** Windows e Linux x64.
- [ ] `rpcs3_libretro` — PlayStation 3. Validar firmware, disco ou pasta PS3, Vulkan e persistência do armazenamento emulado. **Pacote:** Windows e Linux x64.
- [ ] `vita3k_libretro` — PlayStation Vita. Validar firmware e arquivos da versão. Testar VPK, instalação persistente e Vulkan. **Pacote:** Windows e Linux x64.
- [ ] `xemu_libretro` — Xbox. Validar boot, XISO e disco QCOW2, com persistência após fechar. **Pacote:** Windows e Linux x64.
- [ ] `xenia_edge_libretro` — Xbox 360. Validar ISO/XEX e saves em `content/` com Vulkan. Encaminhar D3D12 ao [roteiro de GPU](24-direct3d-por-core.md). **Pacote:** Windows e Linux x64.

### Alternativas para plataformas identificadas (5)

- [ ] `fbalpha2012_neogeo_libretro` — Neo Geo. Testar a seleção do core no sistema já mapeado com `.zip`. **Pacote:** Windows e Linux x64.
- [ ] `fsuae_libretro` — Amiga. Testar a seleção do core no sistema já mapeado com `.adf`, `.ipf`, `.fs-uae`. **Pacote:** Linux x64.
- [ ] `geolith_libretro` — Neo Geo. Testar a seleção do core no sistema já mapeado com `.neo`, `.cue`, `.chd`. **Pacote:** Windows e Linux x64.
- [ ] `pcem_libretro` — PC. Testar a seleção do core no sistema já mapeado com `.exe`, `.com`, `.bat`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `rust_dos_libretro` — DOS. Testar a seleção do core no sistema já mapeado com `.exe`, `.com`, `.bat`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.

### Plataformas e formatos a confirmar (43)

- [ ] `applewin_libretro` — II. Confirmar o mapeamento da plataforma e os formatos do core como `.bin`, `.do`, `.dsk`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `b2_libretro` — BBC Micro. Confirmar o mapeamento da plataforma e os formatos do core como `.ssd`, `.dsd`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `bbkemu_libretro` — BBK Electronic Dictionary. Confirmar o mapeamento da plataforma e os formatos do core como `.gam`. **Pacote:** Windows e Linux x64.
- [ ] `bk_libretro` — BK-0010/BK-0011(M). Confirmar o mapeamento da plataforma e os formatos do core como `.bin`. **Pacote:** Windows e Linux x64.
- [ ] `coolbaremu_libretro` — Coolbar. Confirmar o mapeamento da plataforma e os formatos do core como `.cbe`. **Pacote:** Windows e Linux x64.
- [ ] `dingooemu_libretro` — Dingoo A320. Confirmar o mapeamento da plataforma e os formatos do core como `.app`, `.cc`, `.c2s`. **Pacote:** Windows e Linux x64.
- [ ] `emuscv_libretro` — Super Cassette Vision. Confirmar o mapeamento da plataforma e os formatos do core como `.cart`, `.bin`, `.rom`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `freechaf_libretro` — Channel F. Confirmar o mapeamento da plataforma e os formatos do core como `.bin`, `.chf`. **Pacote:** Windows e Linux x64.
- [ ] `galaksija_libretro` — Galaksija. Confirmar o mapeamento da plataforma e os formatos do core como `.gal`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `gam4980_libretro` — Longman 4980. Confirmar o mapeamento da plataforma e os formatos do core como `.gam`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `geartowns_libretro` — FM Towns. Confirmar o mapeamento da plataforma e os formatos do core como `.cue`, `.chd`, `.iso`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `hyperscanemu_libretro` — HyperScan. Confirmar o mapeamento da plataforma e os formatos do core como `.bin`, `.cue`, `.zip`. **Pacote:** Windows e Linux x64.
- [ ] `m2000_libretro` — P2000T. Confirmar o mapeamento da plataforma e os formatos do core como `.cas`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `mcsoftserve_libretro` — C713. Confirmar o mapeamento da plataforma e os formatos do core. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `minivmac_libretro` — Mac68k. Confirmar o mapeamento da plataforma e os formatos do core como `.dsk`, `.img`, `.zip`. **Pacote:** Windows e Linux x64.
- [ ] `mojozork_libretro` — Z-Machine. Confirmar o mapeamento da plataforma e os formatos do core como `.dat`, `.z1`, `.z3`. **Pacote:** Windows e Linux x64.
- [ ] `mu_libretro` — Palm OS. Confirmar o mapeamento da plataforma e os formatos do core como `.prc`, `.pqa`, `.img`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `native32emu_libretro` — Native32. Confirmar o mapeamento da plataforma e os formatos do core como `.smf`, `.sgm`, `.ssl`. **Pacote:** Windows e Linux x64.
- [ ] `nekop2_libretro` — PC-98. Confirmar o mapeamento da plataforma e os formatos do core como `.d98`, `.zip`, `.98d`. **Pacote:** Windows e Linux x64.
- [ ] `nicaiemu_libretro` — Nicai. Confirmar o mapeamento da plataforma e os formatos do core como `.cbe`. **Pacote:** Windows e Linux x64.
- [ ] `np2kai_libretro` — PC-98. Confirmar o mapeamento da plataforma e os formatos do core como `.d98`, `.zip`, `.98d`. **Pacote:** Windows e Linux x64.
- [ ] `numero_libretro` — TI83. Confirmar o mapeamento da plataforma e os formatos do core como `.8xp`, `.8xk`, `.8xg`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `oberon_libretro` — Oberon RISC machine. Confirmar o mapeamento da plataforma e os formatos do core como `.dsk`. **Pacote:** Windows e Linux x64.
- [ ] `pd777_libretro` — Epoch Cassette Vision. Confirmar o mapeamento da plataforma e os formatos do core como `.zip`, `.bin777`, `.ptn777`. **Pacote:** Windows e Linux x64.
- [ ] `piece_libretro` — P/ECE. Confirmar o mapeamento da plataforma e os formatos do core como `.pfi`. **Pacote:** Windows e Linux x64.
- [ ] `playdiaemu_libretro` — Playdia. Confirmar o mapeamento da plataforma e os formatos do core como `.cue`, `.zip`, `.iso`. **Pacote:** Windows e Linux x64.
- [ ] `pokket_libretro` — PocketStation. Confirmar o mapeamento da plataforma e os formatos do core como `.bin`, `.gme`, `.mcd`. **Pacote:** Windows e Linux x64.
- [ ] `qemu_libretro` — QEMU. Confirmar o mapeamento da plataforma e os formatos do core como `.qemu_cmd_line`, `.iso`, `.img`. **Pacote:** Windows e Linux x64.
- [ ] `quasi88_libretro` — PC-88 series. Confirmar o mapeamento da plataforma e os formatos do core como `.d88`, `.u88`, `.m3u`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `retro8_libretro` — PICO-8. Confirmar o mapeamento da plataforma e os formatos do core como `.p8`, `.png`. **Pacote:** Windows e Linux x64.
- [ ] `spmp8000emu_libretro` — SPMP8000. Confirmar o mapeamento da plataforma e os formatos do core como `.bin`. **Pacote:** Windows e Linux x64.
- [ ] `squirreljme_libretro` — Java ME. Confirmar o mapeamento da plataforma e os formatos do core como `.jar`, `.sqc`, `.jam`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `tamalibretro_libretro` — Tamagotchi P1. Confirmar o mapeamento da plataforma e os formatos do core como `.b`, `.bin`, `.rom`. **Pacote:** Windows e Linux x64.
- [ ] `theodore_libretro` — Thomson MO/TO. Confirmar o mapeamento da plataforma e os formatos do core como `.fd`, `.sap`, `.k7`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `vice_xcbm2_libretro` — CBM-II. Confirmar o mapeamento da plataforma e os formatos do core como `.d64`, `.d71`, `.d80`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `vice_xcbm5x0_libretro` — CBM-5x0. Confirmar o mapeamento da plataforma e os formatos do core como `.d64`, `.d71`, `.d80`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `vice_xpet_libretro` — PET. Confirmar o mapeamento da plataforma e os formatos do core como `.d64`, `.d71`, `.d80`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `vice_xplus4_libretro` — PLUS/4. Confirmar o mapeamento da plataforma e os formatos do core como `.d64`, `.d71`, `.d80`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `vice_xvic_libretro` — VIC-20. Confirmar o mapeamento da plataforma e os formatos do core como `.d64`, `.d71`, `.d80`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `virtualxt_libretro` — PC/XT. Confirmar o mapeamento da plataforma e os formatos do core como `.img`, `.zip`. **Pacote:** Windows e Linux x64.
- [ ] `wqxemu_libretro` — Wenquxing. Confirmar o mapeamento da plataforma e os formatos do core. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `x16emu_libretro` — Commander X16. Confirmar o mapeamento da plataforma e os formatos do core como `.img`, `.prg`, `.bas`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `x1_libretro` — Sharp X1. Confirmar o mapeamento da plataforma e os formatos do core como `.dx1`, `.zip`, `.2d`. **Pacote:** Windows e Linux x64.

### Cores com rota gráfica a validar (11)

- [ ] `3dengine_libretro` — 3D Engine. Confirmar o contexto de hardware anunciado e testar a primeira saída pelo core-host. **Pacote:** Windows e Linux x64.
- [ ] `boom3_libretro` — Doom 3 Game Engine. Confirmar o contexto de hardware anunciado e testar a primeira saída pelo core-host. **Pacote:** Windows e Linux x64.
- [ ] `doukutsu_rs_libretro` — Cave Story Game Engine. Confirmar o contexto de hardware anunciado e testar a primeira saída pelo core-host. Confirmar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `eduke32_libretro` — Build Engine. Confirmar o contexto de hardware anunciado e testar a primeira saída pelo core-host. **Pacote:** Windows e Linux x64.
- [ ] `nuance_libretro` — NUON. Confirmar o contexto de hardware anunciado e testar a primeira saída pelo core-host. **Pacote:** Windows e Linux x64.
- [ ] `omicron_libretro` — Omicron. Confirmar o contexto de hardware anunciado e testar a primeira saída pelo core-host. **Pacote:** Linux x64.
- [ ] `openlara_libretro` — Classic Tomb Raider engine. Confirmar o contexto de hardware anunciado e testar a primeira saída pelo core-host. **Pacote:** Windows e Linux x64.
- [ ] `supermodel_libretro` — Model 3. Confirmar o contexto de hardware anunciado e testar a primeira saída pelo core-host. **Pacote:** Windows e Linux x64.
- [ ] `thepowdertoy_libretro` — Physics Toy. Confirmar o contexto de hardware anunciado e testar a primeira saída pelo core-host. Confirmar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `uzdoom_libretro` — DOOM Game Engine. Confirmar o contexto de hardware anunciado e testar a primeira saída pelo core-host. **Pacote:** Windows e Linux x64.
- [ ] `vircon32_libretro` — Vircon32. Confirmar o contexto de hardware anunciado e testar a primeira saída pelo core-host. Confirmar abertura sem conteúdo. **Pacote:** Windows e Linux x64.

### Engines, jogos e utilitários (32)

- [ ] `2048_libretro` — 2048 Game Clone. Validar abertura sem conteúdo, saída do core e encerramento. **Pacote:** Windows e Linux x64.
- [ ] `anarch_libretro` — Anarch. Validar abertura sem conteúdo, saída do core e encerramento. **Pacote:** Windows e Linux x64.
- [ ] `cannonball_libretro` — Outrun Game Engine. Validar abertura sem conteúdo, saída do core e encerramento. **Pacote:** Windows e Linux x64.
- [ ] `chailove_libretro` — ChaiLove. Definir conteúdo inicial com `.chai`, `.chailove` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `craft_libretro` — Minecraft Game Clone. Validar abertura sem conteúdo, saída do core e encerramento. **Pacote:** Windows e Linux x64.
- [ ] `dinothawr_libretro` — Dinothawr Game Engine. Validar abertura sem conteúdo, saída do core e encerramento. **Pacote:** Windows e Linux x64.
- [ ] `easyrpg_libretro` — RPG Maker 2000/2003 Game Engine. Definir conteúdo inicial com `.ldb`, `.zip`, `.lzh` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `ecwolf_libretro` — Wolfenstein 3D Game Engine. Definir conteúdo inicial com `.wl6`, `.n3d`, `.sod` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `gme_libretro` — Music. Validar a entrada e saída próprias da categoria (ex.: `.ay`, `.gbs`, `.gym`) e verificar o encerramento. **Pacote:** Windows e Linux x64.
- [ ] `gong_libretro` — Pong Game Clone. Validar abertura sem conteúdo, saída do core e encerramento. **Pacote:** Windows e Linux x64.
- [ ] `jumpnbump_libretro` — Jump 'n Bump. Definir conteúdo inicial com `.dat` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `lowresnx_libretro` — LowRes NX. Definir conteúdo inicial com `.nx` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `lutro_libretro` — Lutro. Definir conteúdo inicial com `.lutro`, `.love`, `.lua` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `mrboom_libretro` — Mr.Boom. Validar abertura sem conteúdo, saída do core e encerramento. **Pacote:** Windows e Linux x64.
- [ ] `nxengine_libretro` — Cave Story Game Engine. Validar abertura sem conteúdo, saída do core e encerramento. **Pacote:** Windows e Linux x64.
- [ ] `pocketcdg_libretro` — Music. Validar a entrada e saída próprias da categoria (ex.: `.cdg`) e verificar o encerramento. **Pacote:** Windows e Linux x64.
- [ ] `radio_libretro` — Internet Radio Player. Validar abertura sem conteúdo, saída do core e encerramento. **Pacote:** Windows e Linux x64.
- [ ] `reminiscence_libretro` — Flashback Game Engine. Definir conteúdo inicial com `.map`, `.aba`, `.seq` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `remotejoy_libretro` — PSP. Validar abertura sem conteúdo, saída do core e encerramento. **Pacote:** Windows e Linux x64.
- [ ] `romcleaner_libretro` — ROM Cleaner. Validar a entrada e saída próprias da categoria (ex.: `.sfc`, `.smc`, `.st`) e verificar o encerramento. **Pacote:** Windows e Linux x64.
- [ ] `superbroswar_libretro` — Super Bros War. Definir conteúdo inicial com `.game` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `tyrquake_libretro` — Quake Game Engine. Definir conteúdo inicial com `.pak` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `uw8_libretro` — MicroW8. Definir conteúdo inicial com `.uw8`, `.wasm` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `vemulator_libretro` — SEGA Visual Memory Unit. Definir conteúdo inicial com `.vms`, `.dci`, `.bin` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `video_processor_libretro` — Video Processor. Validar a entrada e saída próprias da categoria e verificar o encerramento. **Pacote:** Linux x64.
- [ ] `vitaquake2-rogue_libretro` — Quake II Game Engine. Definir conteúdo inicial com `.pak` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `vitaquake2-xatrix_libretro` — Quake II Game Engine. Definir conteúdo inicial com `.pak` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `vitaquake2-zaero_libretro` — Quake II Game Engine. Definir conteúdo inicial com `.pak` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `vitaquake2_libretro` — Quake II Game Engine. Definir conteúdo inicial com `.pak` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `vitaquake3_libretro` — Quake 3 Game Engine. Definir conteúdo inicial com `.pk3` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `wasm4_libretro` — WASM-4. Definir conteúdo inicial com `.wasm` e testar o acesso aos dados auxiliares. **Pacote:** Windows e Linux x64.
- [ ] `xrick_libretro` — Rick Dangerous Game Engine. Validar abertura sem conteúdo, saída do core e encerramento. **Pacote:** Windows e Linux x64.

### Pacotes com metadados a confirmar (6)

- [ ] `boom3_xp_libretro` — Metadados a confirmar. Identificar a origem do pacote e coletar o tipo de core, formatos e opções com o probe. **Pacote:** Windows e Linux x64.
- [ ] `fbalpha_libretro` — Metadados a confirmar. Identificar a origem do pacote e coletar o tipo de core, formatos e opções com o probe. **Pacote:** Windows e Linux x64.
- [ ] `linuxloader_libretro` — Metadados a confirmar. Identificar a origem do pacote e coletar o tipo de core, formatos e opções com o probe. **Pacote:** Windows e Linux x64.
- [ ] `n64emu_libretro` — Metadados a confirmar. Identificar a origem do pacote e coletar o tipo de core, formatos e opções com o probe. **Pacote:** Windows e Linux x64.
- [ ] `odin_libretro` — Metadados a confirmar. Identificar a origem do pacote e coletar o tipo de core, formatos e opções com o probe. **Pacote:** Windows e Linux x64.
- [ ] `pokketstation_libretro` — Metadados a confirmar. Identificar a origem do pacote e coletar o tipo de core, formatos e opções com o probe. **Pacote:** Windows e Linux x64.

## IDs oficiais sem pacote nos índices Windows e Linux x64 (95)

Confirmar alvo e disponibilidade por core. Se houver build para outra
arquitetura ou plataforma, registrar esse alvo. Se faltar binário utilizável,
registrar o impedimento e manter a entrada em backlog.

- [ ] `advanced_tests_libretro` — Advanced Test Core. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `anybor_libretro` — OpenBOR. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `anygm_libretro` — GameMaker. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `armsx2_libretro` — Sony PlayStation 2. Confirmar builds e arquitetura do port. Validar PS2 quando o alvo do ReEmu correspondente estiver pronto.
- [ ] `beetle_saturn_libretro` — Saturn. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `bennugd_libretro` — Bennu Game Development. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `bnes_libretro` — Nintendo Entertainment System. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `boytacean_libretro` — Game Boy/Game Boy Color. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `chimerasnes_libretro` — Super Nintendo Entertainment System. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `citra_canary_libretro` — 3DS. Localizar release da variante Canary para o alvo. Levar o binário aos testes de 3DS quando houver pacote.
- [ ] `cruzes_libretro` — Cruzes. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `daphne_libretro` — LaserDisc. Em 2026-11-25, o Buildbot retornou HTTP 404 para os pacotes x64 de Windows e Linux. A compilação Windows x64 com MinGW parou nas chamadas `pthread_create`, `pthread_join` e `pthread_detach` do backend SDL. Pendentes um pacote utilizável e a validação de framefile, ROM e vídeo dessa variante. [Código da revisão testada](https://github.com/libretro/daphne/tree/6f1695dd1f376060666eec0a416ff56bb6c9cccc)
- [ ] `directxbox_libretro` — Xbox. Localizar build do alvo e validar boot experimental com Cromwell. Registrar a dependência upstream para jogos comerciais.
- [ ] `dolphin_launcher_libretro` — GameCube / Wii. Identificar o alvo e o papel do launcher na origem. Validar abertura como um ID distinto do Dolphin libretro.
- [ ] `dosbox_svn_ce_libretro` — DOS. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `duckstation_libretro` — PlayStation. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `eka2l1_libretro` — Symbian OS / N-Gage. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `emucv_libretro` — Cassette Vision. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `emux_chip8_libretro` — CHIP-8. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `emux_gb_libretro` — Game Boy/Game Boy Color. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `emux_nes_libretro` — Nintendo Entertainment System. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `emux_sms_libretro` — Sega Master System. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `eternal_libretro` — Eternal Computer. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `fake08_libretro` — PICO-8. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `fbneo_cps12_libretro` — CP System I/II. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `fbneo_neogeo_libretro` — Neo Geo. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `ffmpeg_libretro` — FFmpeg. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `flycast_gles2_libretro` — Sega Dreamcast. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `FreeIntvTSOverlay_libretro` — Intellivision. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `freej2me_libretro` — J2ME. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `freej2me_plus_libretro` — J2ME. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `higan_sfc_libretro` — Super Nintendo Entertainment System. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `imageviewer_libretro` — Imageviewer. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `ishiiruka_libretro` — GameCube / Wii. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `jgenesis_libretro` — Multi System. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `mame2009_libretro` — Arcade (various). Confirmar alvo, origem e disponibilidade do binário.
- [ ] `mamearcade_libretro` — Arcade (various). Confirmar alvo, origem e disponibilidade do binário.
- [ ] `mamemess_libretro` — Multi (various). Confirmar alvo, origem e disponibilidade do binário.
- [ ] `mess2015_libretro` — Multi (various). Confirmar alvo, origem e disponibilidade do binário.
- [ ] `mgba_splitscreen_libretro` — Game Boy Advance (2-4 linked players). Confirmar alvo, origem e disponibilidade do binário.
- [ ] `mkxp-z_libretro` — RPG Maker XP/VX/VX Ace Game Engine. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `moonlight_libretro` — Moonlight. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `mpv_libretro` — MPV. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `mupen64plus_next_develop_libretro` — Nintendo 64. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `mupen64plus_next_gles2_libretro` — Nintendo 64. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `mupen64plus_next_gles3_libretro` — Nintendo 64. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `nes_libretro` — Nintendo Entertainment System. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `onscripter_libretro` — ONScripter. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `onsyuri_libretro` — ONScripter Yuri. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `opentyrian_libretro` — Tyrian Game Engine. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `parallel_n64_debug_libretro` — Nintendo 64. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `parallel_psp_libretro` — PSP. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `parallel_xbox_libretro` — Xbox. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `pascal_pong_libretro` — PascalPong. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `pcsx1_libretro` — PlayStation. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `pcsx_rearmed_interpreter_libretro` — PlayStation. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `pcsx_rearmed_neon_libretro` — PlayStation. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `puzzlescript_libretro` — PuzzleScript. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `redbook_libretro` — Redbook. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `retrodream_libretro` — Sega Dreamcast. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `rp6502_libretro` — Picocomputer 6502. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `rustation_libretro` — PlayStation. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `rvvm_libretro` — RVVM. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `sdlpal_libretro` — SDLPAL. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `simcp_libretro` — SAM coupe. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `sm2_libretro` — Model 2. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `stonesoup_libretro` — Dungeon Crawl Stone Soup. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `syobonaction_libretro` — SyobonAction. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `tempgba_libretro` — Game Boy Advance. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `test_libretro` — Test. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `test_netplay_libretro` — Test for netplay. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `testaudio_callback_libretro` — TestAudio Callback. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `testaudio_no_callback_libretro` — TestAudio NoCallback. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `testaudio_playback_wav_libretro` — TestAudio Playback Wav. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `testgl_compute_shaders_libretro` — TestGL ComputeShaders. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `testgl_ff_libretro` — TestGL (FF). Confirmar alvo, origem e disponibilidade do binário.
- [ ] `testgl_libretro` — TestGL. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `testinput_buttontest_libretro` — Button Test. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `testretroluxury_libretro` — Test RetroLuxury. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `testsw_libretro` — TestSW. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `testsw_vram_libretro` — TestSW VRAM. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `testvulkan_async_compute_libretro` — TestVulkan AsyncCompute. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `testvulkan_libretro` — TestVulkan. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `tgpulse_next_m1_libretro` — Model 1. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `trident_libretro` — 3DS. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `uae4arm_libretro` — Amiga. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `ume2015_libretro` — Multi (various). Confirmar alvo, origem e disponibilidade do binário.
- [ ] `uxn_libretro` — Uxn / Varvara. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `vice_x64dtv_libretro` — C64DTV. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `vidtest_libretro` — VidTest. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `vitavoyager_libretro` — Quake 3 Game Engine. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `x64sdl_libretro` — C64. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `yaps2_libretro` — Sony PlayStation 2. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `zcdi_libretro` — CD-i. Confirmar alvo, origem e disponibilidade do binário.
- [ ] `zesarpcw_libretro` — PCW. Confirmar alvo, origem e disponibilidade do binário.

## Cores já no catálogo

Os testes pendentes em Linux x64 estão reunidos na
[tarefa de validação em Linux](../../TASKS.md#testes-em-linux-x64).

| Core | Evidência no Windows x64 | Linux x64 e modos adicionais |
| --- | --- | --- |
| `dolphin_libretro` | GameCube integrado inicialmente ao catálogo com OpenGL e instalação dos arquivos `Sys`. A validação interativa do piloto segue no [plano Dolphin](25-dolphin-libretro.md). | Pacote Linux x64 publicado. Validação em Linux e integração do Wii pendentes. |
| `fbalpha2012_cps1_libretro` | A DLL `v0.2.97.28` abre os ZIPs de CPS-1 pela rota Arcade, com o arquivo inteiro e o nome original. Cinco conjuntos locais rodaram 1.800 quadros de 384×224 e restauraram save states com a pasta de sistema vazia. Houve áudio audível em três títulos. Quatro outros ZIPs da biblioteca foram recusados. A seleção automática preserva a ordem dos outros candidatos Arcade que também aceitam ZIP. | Pacote Linux x64 publicado. Execução em Linux e teste interativo de controle pendentes. [Documentação oficial e versão do romset](https://docs.libretro.com/library/fbalpha2012_cps1/) |
| `fbalpha2012_cps2_libretro` | A DLL `v0.2.97.28` abre ZIPs do CP System II pela rota Arcade. Cinco conjuntos locais rodaram 1.800 quadros de 384×224 e restauraram save states usando o ZIP original e uma pasta de sistema vazia. Os cinco enviaram amostras de áudio e três emitiram amostras audíveis nesse intervalo. A fumaça aceita `REEMU_CPS2_ROM` para repetir a execução com um conjunto compatível. | Pacote Linux x64 publicado. Execução em Linux e teste interativo de controle pendentes. [Documentação oficial](https://docs.libretro.com/library/fbalpha2012_cps2/) |
| `fbalpha2012_cps3_libretro` | A DLL `v0.2.97.29` abriu seis conjuntos locais de CP System III em ZIP pela rota Arcade. Cada conjunto rodou 1.800 quadros de 384×224 com áudio audível e restauração de save state, usando uma pasta de sistema vazia. Um ZIP de Street Fighter III sem o arquivo com CRC `fb172a8e` foi recusado. Outra cópia local do jogo passou. A fumaça aceita `REEMU_CPS3_ROM`. | A DLL anuncia `.iso` e `.zip`. A validação cobriu ZIP no Windows x64. Teste interativo de controle, ISO e execução em Linux seguem pendentes. [Documentação oficial](https://docs.libretro.com/library/fbalpha2012_cps3/) |
| `amiarcadia_libretro` | Buildbot de 2026-11-26, versão `4.60`, licença não comercial. Programas de teste próprios abriram sem BIOS externa. No app, passaram instalação, varredura e abertura de Arcadia 2001 e Interton VC 4000 em `.bin`, e Elektor em `.tvc`, inclusive ZIP e 7z. O RetroPad A alterou a imagem no Arcadia e o estado foi restaurado após fechar e reabrir. A fumaça confirma vídeo, tom e restauração de estado com `.bin`, `.tvc` e ZIP. | Pacote Linux x64 publicado. Execução em Linux e integração dos arcades Zaccaria e Malzak ficam pendentes. [Documentação](https://docs.libretro.com/library/amiarcadia/) · [Código consultado](https://github.com/warmenhoven/amiarcadia/tree/8ca27c925833fe0021475155512c7f467924f647) |
| `bsnes-jg_libretro` | Buildbot de 2026-11-23, versão 2.1.0. Descoberta e probe pelo loader. A fumaça do catálogo rodou 180 quadros com imagem, áudio e restauração de save state. Teste adicional confirmou o botão B do RetroPad e a restauração de 2 KiB de SRAM após reabertura. | Pacote publicado. Validação em Linux e dos subsistemas Super Game Boy, BS-X e Sufami Turbo pendente. |
| `mesen-s_libretro` | Buildbot de 2026-11-23, versão 0.4.0. O core-host rodou 180 quadros com imagem, áudio e restauração de save state para `.sfc`, `.gb` e `.gbc` com modo Color obrigatório. Testes adicionais confirmaram o botão B do RetroPad, a restauração da SRAM após reabertura, o modo Super Game Boy com as duas revisões de firmware e a inicialização de Satellaview com `.bs` avulso e em ZIP. | Pacote publicado. Execução em Linux e validação de jogos comerciais de Satellaview pendentes. [Documentação oficial](https://docs.libretro.com/library/mesen-s/) |
| `mesen2_libretro` | Buildbot de 2026-11-23, versão 2.0.0 anunciada pela DLL. O core-host rodou 180 quadros com imagem, áudio e restauração de save state em cada cartucho de teste de NES, SNES, GB, GBC, GBA, PC Engine, SuperGrafx, Master System, Game Gear, WonderSwan e WonderSwan Color. O GBA abriu sem BIOS com a opção de iniciar direto no cartucho aplicada pelo ReEmu. FDS e PC Engine CD passaram com mídia sintética e também com programas homebrew usando firmware original. Super Game Boy 2 passou com firmware sintético e original. | Pacote Linux verificado como ELF64 x86-64 com os símbolos Libretro. Execução em Linux e boot de discos comerciais pendentes. [Documentação oficial](https://docs.libretro.com/library/mesen2/) |
| `DoubleCherryGB_libretro` | Buildbot de 2026-11-24, versão `v0.19.0 d432f97`. O core-host rodou `.gb`, `.gbc` em modo Color obrigatório e `.gbc` dentro de ZIP, com 180 quadros de 160×144, áudio e restauração de save state, usando pasta de BIOS vazia. O loader registrou o botão B do RetroPad e restaurou 8 KiB de SRAM em outro processo. A licença do catálogo vem do [arquivo da revisão da DLL](https://git.libretro.com/libretro/doublecherryGB-libretro/-/blob/d432f97/LICENSE). | Pacote publicado para Linux x64. Execução em Linux e multiplayer pelo cabo Link pendentes. [Projeto oficial](https://git.libretro.com/libretro/doublecherryGB-libretro) |
| `hatari_libretro` | Buildbot de 2026-11-24, versão `2.6.1-devel (Nov 24 2026)`. O core-host executou o setor de boot de um disquete Atari ST com EmuTOS 1.4 em `.st`, `.msa`, ZIP, 7z e lista `.m3u`, com 360 quadros de 320×200, áudio e restauração de estado. O aplicativo instalou o core, reconheceu e importou o firmware pelo MD5, abriu o disco ZIP pela biblioteca, acionou o teclado virtual por RetroPad X e restaurou um save state após reabrir o jogo. | Validação em Linux, teclado físico, troca de disco no menu do ReEmu e demais modelos/formatos pendentes. [Projeto oficial](https://github.com/libretro/hatari) · [Pacote Linux x64](https://buildbot.libretro.com/nightly/linux/x86_64/latest/) |
| `hatari2014_libretro` | Buildbot de 2026-11-24, versão `1.8 ab55c3e`. Testado no core-host com o mesmo EmuTOS em `.st`, `.msa`, ZIP, 7z e `.m3u`. O programa do disquete produziu a imagem esperada em 320×200, com áudio e restauração de estado. O core recusou o conteúdo sem `tos.img`. O app instalou a variante pelo catálogo e abriu o ZIP pela biblioteca após selecioná-la como core da plataforma. O save state foi restaurado depois de fechar e reabrir o jogo. Com Automático e os dois cores instalados, o ZIP abriu pelo Hatari atual. | Validação em Linux, teclado e joystick específicos da variante e troca de disco no menu do ReEmu pendentes. [Core Info](https://github.com/libretro/libretro-core-info/blob/master/hatari2014_libretro.info) |
| `hatarib_libretro` | Buildbot de 2026-11-24, versão `v1.1 5f05733`. O core-host executou o setor de boot em `.st`, `.msa`, ZIP, `.m3u` e `.m3u8` com EmuTOS embutido. Foram 360 quadros de 384×268, áudio e restauração de estado em cada formato. No app, passaram download, scan, abertura de ZIP sem BIOS externa, teclado virtual por RetroPad L1, save state após reabertura e seleção de STE em Configurações. | Validação em Linux, troca de disco pelo menu, boot sem conteúdo, teclado e mouse físicos e outros modelos e mídias pendentes. [Documentação do hatariB](https://docs.libretro.com/library/hatarib/) |
| `px68k_libretro` | Versão `0.15+ 3f4f542` do Buildbot. O core-host executou o setor de boot em XDF, DIM, ZIP e M3U com 359 quadros de 768×512, áudio e restauração de estado em cada formato. Um ZIP com dois discos e uma M3U com o nome da entrada interna também abriram o disco esperado. No app, passaram a instalação pelo catálogo, a importação de firmware renomeado por MD5, a abertura do ZIP pela biblioteca, o menu nativo com RetroPad L2 e a restauração de save state após reabrir o jogo. | Pacote publicado para Linux x64. A validação em Linux, a gravação em disquetes, o boot com HDF e a troca de disco pelo menu do ReEmu seguem pendentes. [Documentação do PX68k](https://docs.libretro.com/library/px68k/) |
| `dirksimple_libretro` | Buildbot de 2026-11-24, versão `0.3`. O core-host executou 120 quadros de 320×240 em OGV e DIRKSIMPLE com vídeo Theora sintético, áudio Vorbis e restauração do save state. No app, passaram download dos dados do Buildbot, scan, abertura pela biblioteca, botão A do RetroPad e restauração de estado após reabrir o jogo. | Pacote publicado para Linux x64. A rodada Linux e a validação com vídeos completos dos jogos ficam pendentes. [Documentação do DirkSimple](https://docs.libretro.com/library/dirksimple/) |
| `hypseushw_libretro` | Versão `0.2.0 (Hypseus Singe 3.0.2-libretro)`. O core-host abriu a mesma mídia de teste por framefile e ZIP, apresentou vídeo MPEG-2 em OpenGL e entregou amostras do áudio Vorbis. Na interface, passaram instalação, seleção pelo framefile com os dois cores LaserDisc instalados, RetroPad B, criação e reutilização do índice e abertura em CPU Fallback. A mídia usa um programa Z80 próprio e vídeo sintético. O loader retornou o erro do core ao faltar o `.m2v`. | Linux, Vulkan, Singe e jogos completos ficam pendentes. O core oferece persistência própria conforme o jogo. [Documentação do HypseusHW](https://docs.libretro.com/library/hypseushw/) |
| `openmsx_libretro` | Versão `21.0 9dcdb1ed14` do Buildbot. O core-host executou 450 quadros com vídeo, áudio e restauração de save state em cartucho MSX2 avulso e em ZIP Deflate, com C-BIOS embutido. No app, passaram instalação, scan, abertura do ZIP e restauração de save state após reabrir o jogo. Uma máquina C-BIOS de teste com drive e firmware sintético montou um DSK diretamente e por M3U. | Pacote Linux x64 publicado. Execução em Linux, boot com disquetes e firmware de máquinas reais, fitas, teclado físico e troca de disco pelo menu do ReEmu pendentes. [Documentação do openMSX](https://docs.libretro.com/library/openmsx/) |
| `jaxe_libretro` | Versão `1.0 c767afd` do Buildbot. A biblioteca agrupa `.ch8`, `.sc8`, `.xo8` e `.hc8` em **CHIP-8**. A fumaça rodou os quatro formatos avulsos e uma ROM em ZIP com vídeo, áudio e save state. O core-host também abriu uma ROM em 7z. No app, passaram instalação, abertura do ZIP, botão B do RetroPad, persistência dos flags em save RAM e restauração de save state após reabrir o jogo. O core usa renderização por software e dispensa BIOS externa. | Pacote Linux x64 publicado. A execução em Linux está no backlog. [Documentação do JAXE](https://docs.libretro.com/library/jaxe/) · [Extensões no código da revisão](https://git.libretro.com/libretro/jaxe/-/blob/c767afd785e01a15bcd575a2d93b737add82b675/src/libretro.c) |
| `ardens_libretro` | Versão `v0.25.13` do Buildbot. O core-host executou um programa AVR próprio como `.hex` e como pacote `.arduboy` com vídeo, áudio e save state. No app, passaram instalação, scan, abertura dos dois formatos, botão B do RetroPad, leitura do flash FX empacotado e restauração da EEPROM e do save state após reabrir. O scan reconhece `.arduboy` diretamente e `.hex` em pastas **Arduboy** ou **Arduboy FX**. O core dispensa BIOS externa. | Pacote Linux x64 publicado. Execução em Linux e gravação de dados no flash FX pendentes. [Documentação do Ardens](https://docs.libretro.com/library/ardens/) · [Código da revisão](https://github.com/tiberiusbrown/Ardens/blob/0af7b20e99fc98352c3f2eab8d76a50cbf6613d2/src/libretro_core/libretro_impl.cpp) |
| `arduous_libretro` | Versão `0.1.0` do Buildbot. A fumaça compartilha o programa AVR `.hex` do Ardens e verifica vídeo, áudio e restauração do save state. No app Windows, passaram instalação, botão B do RetroPad e restauração de EEPROM e save state após reabrir. O Arduous pode ser escolhido para `.hex`. Pacotes `.arduboy` abriram pelo Ardens mesmo com Arduous como padrão da plataforma. A EEPROM de 1 KiB fica em `saves/arduous_libretro`, separada do `.srm` do Ardens. Um `.srm` legado de 1 KiB foi reaproveitado no primeiro uso e preservado no caminho original. | Execução em Linux e sincronização da EEPROM do Arduous pendentes. [Documentação do Arduous](https://docs.libretro.com/library/arduous/) · [Código upstream](https://github.com/libretro/arduous/blob/831b83e67ad46e8c86b8165649ff81bc6fc6c157/src/libretro/libretro.cpp) |
| `uzem_libretro` | Versão `v2.0` do Buildbot. No app Windows, passaram instalação, importação e abertura de `.uze` avulso, ZIP e 7z sem BIOS externa. O teste confirmou restauração do save state e da EEPROM de 2 KiB após reabrir, além da montagem do cartão SD a partir de `system/uzem`. A fumaça com `REEMU_SMOKE_ONLY=uzem_libretro` e `REEMU_UZEM_ROM` verifica imagem, áudio e save state. | Pacote Linux x64 publicado. Execução em Linux no backlog. [Documentação do Uzem](https://docs.libretro.com/library/uzem/) · [Código upstream e licença MIT](https://github.com/libretro/libretro-uzem/blob/d991ee94547c8294abc1c4cb73d63116aa58b5bc/uzem_libretro.cpp) · [ROM de teste do Uzebox](https://github.com/Uzebox/uzebox/blob/ae1dc1d1d98f7205642bbbf88be4e801d1832676/tools/packrom/Test/arkanoid.uze) |
| `vaporspec_libretro` | Versão `1.0 11f6384` do Buildbot. No app Windows, passaram instalação, abertura de `.vaporbin` avulso, ZIP e 7z sem BIOS externa, botão B do RetroPad e restauração de save states após reabrir. A fumaça com `REEMU_SMOKE_ONLY=vaporspec_libretro` verifica o vídeo de 256 × 192 e a restauração do estado com uma ROM sintética. | A versão testada envia amostras silenciosas. O pacote Linux x64 está publicado e a execução ficou no backlog. [Documentação do VaporSpec](https://docs.libretro.com/library/vaporspec/) · [Código upstream](https://github.com/minkcv/vm/blob/11f63848a592644722ea1240b4b96785db9b009f/machine/libretro.c) |
| `gametank_libretro` | Versão `1.0.3 b9d0436` do Buildbot. No app Windows, passaram instalação, abertura de `.gtr` avulso, ZIP e 7z sem BIOS externa, RetroPad e restauração de save states após reabrir. Cartuchos de teste confirmaram a persistência local de `.xor` nos três formatos e de `.srm` de 32 KiB em um cartucho `SAVE`. O `.xor` usa o nome do arquivo da biblioteca. A fumaça `REEMU_SMOKE_ONLY=gametank_libretro` com `REEMU_GAMETANK_ROM` verifica vídeo, áudio e save state. | Pacote Linux x64 publicado. Execução em Linux no backlog. [Documentação do GameTank](https://docs.libretro.com/library/gametank/) · [Código upstream](https://github.com/SundownerSport/gametank-libretro/blob/b9d0436ba70786912e9ef2dda9b3b3ee8e1c3c13/src/libretro.cpp) |
| `sameduck_libretro` | Versão `0.13.6 040e19c` do Buildbot. No app Windows, passaram instalação sem BIOS externa, abertura de `.duck` e `.bin` avulsos, ZIP e 7z, botão B do RetroPad e restauração de save states após reabrir. O scan reconhece `.duck` diretamente e `.bin` em pastas Mega Duck ou Cougar Boy. O `.duck` tem prioridade quando ambos estão no mesmo arquivo compactado. A fumaça `REEMU_SMOKE_ONLY=sameduck_libretro` verifica vídeo, áudio e save state com um cartucho próprio. | A revisão testada expõe save states e retorna tamanho zero para SRAM de cartucho. Pacote Linux x64 publicado e validação em execução no backlog. [Documentação do SameDuck](https://docs.libretro.com/library/sameduck/) · [Código upstream](https://github.com/libretro/SameDuck/blob/040e19c379f00dae10afe0eb0c1b2b019b985bbe/libretro/libretro.c) |
| `gw_libretro` | Versão `1.6.3` do Buildbot. No app Windows, passaram instalação, abertura de `.mgw` avulso, ZIP e 7z, mapa de controles pelo Start e restauração dos 777 bytes de SRAM após reabrir. O core-host executa o primeiro quadro após restaurar a SRAM e usa a geometria do simulador para dimensionar o anel de vídeo. A fumaça `REEMU_SMOKE_ONLY=gw_libretro` com `REEMU_GW_ROM` verifica vídeo e envio de amostras de áudio. | Save states indisponíveis nesta versão. A sessão validada produziu amostras silenciosas. Pacote Linux x64 publicado e execução em backlog. [Documentação do GW](https://docs.libretro.com/library/gw/) · [Código upstream](https://github.com/libretro/gw-libretro/blob/dddc9d553f7503f17c3dfd1906bc94c07eff8515/src/libretro.c) |
| `81_libretro` | Versão `1.0a fa70949` do Buildbot. O core-host abriu `.p` avulso, ZIP e 7z com vídeo, áudio e save state. Na interface, passaram instalação, varredura, abertura de ZIP, teclado virtual via Select, restauração após reabrir e abertura de `.tzx` pelo EightyOne com Fuse instalado. A biblioteca reconhece `.p` pelo cabeçalho e `.t81` pela extensão. Uma fita `.tzx` avulsa ou compactada em uma pasta `ZX81` ou `Sinclair - ZX 81` entra em **Sinclair ZX81**. A página do jogo oferece o core do sistema identificado. | Na fita `.t81` gerada com o conversor oficial, a sessão abriu pela interface do Windows e falhou ao gravar o save state. Essa persistência, teclado físico e execução em Linux seguem pendentes. [Documentação do EightyOne](https://docs.libretro.com/library/eightyone/) · [Código da revisão](https://github.com/libretro/81-libretro/tree/fa7094910d040baa5fd8b11dbf6a1a618330ecd9) |

### Atari ST no Windows

O Hatari recebe o caminho do conteúdo. A biblioteca reconhece `.st` pelo
cabeçalho e tamanho, `.msa` e `.stx` pela extensão e usa a pasta da plataforma
para formatos compartilhados como `.ipf`, `.dim`, `.vhd`, `.ide`, `.gem` e
`.m3u`. A regra de `.st` mantém cartuchos Sufami Turbo em Super Nintendo.
Em ZIP e 7z, a seleção examina a entrada interna. O hatariB lê ZIP
diretamente e aceita listas `.m3u8` em pastas Atari ST. Para 7z, use
Hatari ou Hatari 2014. Nessas duas variantes, use uma imagem avulsa para
gravar no disquete, pois arquivos compactados são extraídos a um caminho
temporário.

Em **Configurações › BIOS**, o Atari ST lista `tos.img`. O scan reconhece o
MD5 do Atari TOS 1.02 US e o do `etos192us.img` da distribuição EmuTOS 1.4 de
192 KiB. A importação grava o arquivo como `tos.img`. O aviso de BIOS na
página do jogo consulta o core selecionado. O Hatari e o Hatari 2014
requerem TOS externo. O hatariB inicia com EmuTOS embutido e aceita o
arquivo externo quando presente. A fumaça usa `REEMU_HATARI_TOS` nas
duas primeiras variantes e uma pasta vazia para o hatariB.

O Hatari 2014 anuncia `ZIP`, mas a abertura direta do pacote testado iniciou
apenas o firmware. O loader extrai a entrada reconhecida antes de abrir essa
variante. A biblioteca usa a extensão interna de `.st`, `.msa` ou `.stx` na
escolha do core, e a descoberta normaliza as extensões anunciadas em maiúsculas.
[Core Info](https://github.com/libretro/libretro-core-info/blob/master/hatari_libretro.info) ·
[EmuTOS 1.4](https://sourceforge.net/projects/emutos/files/emutos/1.4/) ·
[Formato ST](https://github.com/hatari/hatari/blob/main/src/floppies/st.c).

O hatariB anuncia `ENDIAN_DEPENDENT` com um ponteiro para uma constante
em `SET_SERIALIZATION_QUIRKS`. Quando o valor das flags já corresponde à
resposta, o loader devolve sucesso sem escrever no ponteiro. Isso evita a
falha na inicialização e mantém a negociação de flags nos outros cores.
[Fonte do hatariB na revisão testada](https://github.com/bbbradsmith/hatariB/blob/5f05733fc21903dce7d978bd0d7aae445ffe1185/core/core.c) ·
[Contrato Libretro](https://raw.githubusercontent.com/libretro/libretro-common/master/include/libretro.h).

### Sharp X68000 no Windows

Instale o PX68k em **Configurações › Cores**. Em **Configurações › BIOS**,
importe o `iplrom.dat` e o `cgrom.dat`. O scan reconhece os arquivos pelo
MD5 e grava em `system/keropi`. A fumaça usa `REEMU_PX68K_BIOS_DIR`
apontando para uma pasta com esses dois arquivos. A rodada usou o IPL
original e a fonte de substituição baseada na Shinonome do WebX68k. O
loader também aceita as variantes de IPL e de fonte que o core procura.
[Lista de BIOS](https://docs.libretro.com/library/px68k/#bios) ·
[Firmware do WebX68k](https://github.com/uraraworks/WebX68k/blob/master/README.md)

A biblioteca distingue XDF pelo tamanho da imagem e DIM pelo cabeçalho e
mapa de trilhas. Para extensões compartilhadas como IMG, HDF e M3U, use
uma pasta da plataforma. Cada ZIP Sharp X68000 entra na biblioteca como
um único item, pois o próprio PX68k monta os discos do pacote. Uma M3U pode
apontar para uma entrada específica com `arquivo.zip#disquete.dim`.
Extraia arquivos 7z antes de jogar ou converta-os em ZIP.
[Carregamento no core](https://github.com/libretro/px68k-libretro/blob/3f4f542a39a89ca63c626326fa50c4f0ea5871d7/libretro.c) ·
[Leitura de disquetes pelo core](https://github.com/libretro/px68k-libretro/blob/3f4f542a39a89ca63c626326fa50c4f0ea5871d7/x68k/fdd.c)

### LaserDisc com DirkSimple no Windows

Instale **DirkSimple** em **Configurações › Cores**. O ReEmu baixa os
dados auxiliares do Buildbot para `system/DirkSimple` e preserva os arquivos
já presentes nessa pasta. Na primeira abertura de um jogo oficial cuja
pasta ainda não tenha o script, o ReEmu tenta instalar o pacote novamente.
Para Dragon's Lair e Cliff Hanger, adicione os seus vídeos `lair.ogv` e
`cliff.ogv` à biblioteca. Use Ogg Theora com áudio Vorbis. O core também
aceita os mesmos contêineres com a extensão `.dirksimple`. Os nomes são
interpretados até o primeiro ponto. [Dados auxiliares do Buildbot](https://buildbot.libretro.com/assets/system/) ·
[Instruções do mantenedor](https://github.com/icculus/DirkSimple#libretro-core)

Arquivos `.ogv` com outros nomes entram no scan em uma pasta `LaserDisc`.
Para um jogo com dados próprios, a pasta `system/DirkSimple/data/games/<id>`
deve conter `game.lua` ou `game.luac`. O loader usa mídia avulsa e a
biblioteca identifica esses vídeos por nome e tamanho. O relógio usa o
tempo entre quadros durante a partida. Ao iniciar, retomar da pausa ou
avançar quadros fora do loop em tempo real, o core recebe seu intervalo de
referência. [Código do core](https://github.com/icculus/DirkSimple/blob/main/dirksimple_libretro.c) ·
[Contrato `SET_FRAME_TIME_CALLBACK`](https://raw.githubusercontent.com/libretro/libretro-common/master/include/libretro.h)

Para rodar a fumaça do catálogo, gere um vídeo sintético como o do exemplo
abaixo e aponte `REEMU_DIRKSIMPLE_OGV` para esse arquivo. Use
`REEMU_SMOKE_ONLY=dirksimple_libretro` para testar só esse core. O teste
usa as duas extensões e confere se a imagem avançou para o segundo trecho,
se há PCM com amostras diferentes de zero e se o core restaura o estado.
[FFmpeg](https://www.ffmpeg.org/download.html)

```sh
ffmpeg -f lavfi -i 'color=c=cyan:s=320x240:r=30:d=1' \
  -f lavfi -i 'color=c=magenta:s=320x240:r=30:d=19' \
  -f lavfi -i 'sine=frequency=660:sample_rate=44100:duration=20' \
  -filter_complex '[0:v][1:v]concat=n=2:v=1:a=0[v]' -map '[v]' -map 2:a \
  -c:v libtheora -q:v 6 -c:a libvorbis reemu-smoke.ogv
```

### LaserDisc com HypseusHW no Windows

Instale **HypseusHW** em **Configurações › Cores** e adicione a pasta do
jogo à biblioteca mantendo a estrutura do Daphne. No exemplo, a primeira
linha de `lair.txt` é `../vldp/lair` e o primeiro registro de quadro pode
ser `0 video.m2v`. O áudio Vorbis usa o mesmo nome-base do vídeo.
[Estrutura do conteúdo](https://docs.libretro.com/library/hypseushw/#usage)

```text
Daphne/
  framefile/lair.txt
  roms/lair.zip
  vldp/lair/video.m2v
  vldp/lair/video.ogg
```

A biblioteca usa o framefile como entrada e preserva o caminho original
durante a abertura para o core encontrar os arquivos associados. Também
reconhece framefiles MPEG em `vldp/<jogo>/` e em pastas `.daphne`. O jogo
usa o core LaserDisc que aceita sua extensão, inclusive quando o padrão
da plataforma aponta para o outro core. O catálogo registra a rota
OpenGL validada no Windows. A opção **CPU Fallback** pertence ao próprio
core. A primeira abertura pode levar mais tempo enquanto o core indexa o
vídeo. O ReEmu apresenta a mensagem de erro de carga enviada pelo core.
[Código do core](https://github.com/Reznnate/hypseushw/blob/cb4de9f9b3e8264e71b25b5552dfa1e26121c636/libretro/lr_core.cpp) ·
[OpenGL 3.2 Core](https://registry.khronos.org/OpenGL/specs/gl/glspec32.core.pdf) ·
[Contrato Libretro](https://raw.githubusercontent.com/libretro/libretro-common/master/include/libretro.h)

A fumaça cria sua própria ROM de teste e a estrutura acima. Gere os dois
arquivos abaixo numa pasta, aponte `REEMU_HYPSEUS_MEDIA_DIR` para ela e
selecione `REEMU_SMOKE_ONLY=hypseushw_libretro`. O teste verifica abertura
por framefile e ZIP, cor do quadro e áudio. [FFmpeg](https://www.ffmpeg.org/download.html)

```sh
ffmpeg -f lavfi -i 'color=c=magenta:s=320x240:r=24000/1001:d=20' \
  -c:v mpeg2video -pix_fmt yuv420p -g 15 -f mpeg2video color.m2v
ffmpeg -f lavfi -i 'sine=frequency=660:sample_rate=44100:duration=20' \
  -ac 2 -c:a libvorbis color.ogg
```

### openMSX no Windows

Instale **openMSX** em **Configurações › Cores** e selecione-o no jogo ou
como core padrão de **MSX** em **Gerenciar biblioteca**. Use uma pasta
`MSX`, `MSX2`, `MSX2+` ou `MSX Turbo R` para reconhecer extensões ambíguas
como `.rom`, `.dsk` e `.cas`. Guarde um único jogo por ZIP, com compressão
Deflate. Mantenha a lista `.m3u` e os disquetes avulsos na estrutura usada por
seus caminhos relativos. [Formatos do core](https://docs.libretro.com/library/openmsx/#extensions)

O modelo padrão `C-BIOS_MSX2+` abre cartuchos sem firmware adicional. Abra
um cartucho com openMSX para que o ReEmu carregue as opções do core. Para
disquetes, selecione nessas opções uma máquina com drive, como a Philips
NMS 8250, e coloque as ROMs de sistema do modelo em `system/openmsx/share/systemroms`
na pasta de dados do ReEmu. O openMSX identifica esses arquivos pelo SHA-1.
[Requisitos do core](https://docs.libretro.com/library/openmsx/#bios) · [Guia de instalação](https://openmsx.org/manual/setup.html)

A fumaça cria o próprio cartucho Z80 e pode ser selecionada com
`REEMU_SMOKE_ONLY=openmsx_libretro`.

### Satellaview e Sufami Turbo no Windows

A biblioteca agrupa `.bs` e cartuchos `.st` válidos em Super Nintendo,
inclusive em ZIP e 7z. Arquivos `.st` passam pela assinatura e pelo
tamanho de cartucho Sufami Turbo para distinguir disquetes Atari ST.
Quando o jogo e a plataforma seguem a escolha automática, a seleção de
core prioriza o Snes9x e considera a extensão interna dos arquivos compactados.
No Mesen2, o loader repassa em memória os arquivos extraídos de `.bs`
e `.st` porque essas extensões não estão no override de conteúdo do core.
[Porta Libretro do MesenCE](https://github.com/libretro/MesenCE/blob/master/Libretro/libretro.cpp) ·
[Identificação no Snes9x](https://github.com/snes9xgit/snes9x/blob/master/libretro/libretro.cpp)

O homebrew `bs-demo-mpack.bs` abriu no Mesen2, Mesen-S e Snes9x, avulso
e em ZIP, com a edição traduzida DRM-Free de `BS-X.bin`. Foram produzidos
600 quadros, saída PCM e save states restauráveis. A captura do Mesen2
confirmou a tela de criação de personagem do BS-X.
[Homebrew de Krokodyl](https://github.com/Krokodyl/bsdemo-mpack) ·
[BIOS no System.dat](https://github.com/libretro/libretro-database/blob/master/dat/System.dat)

O cartucho sintético Sufami Turbo desenhou a tela vermelha do programa
no Mesen2 e no Snes9x, avulso e em ZIP, com restauração de save state.
O Mesen2 carrega o firmware como `SufamiTurbo.sfc` e o Snes9x usa
`STBIOS.bin`. Ambos os nomes estão na busca de BIOS. A associação de
dois cartuchos e os subsistemas do bsnes-jg seguem pendentes.
[Firmware no MesenCE](https://github.com/libretro/MesenCE/blob/master/Core/Shared/FirmwareHelper.h) ·
[Documentação oficial do Snes9x](https://docs.libretro.com/library/snes9x/)

### Mesen2 e Mesen-S

A fumaça do Mesen 2 roda FDS e PC Engine CD no core-host com dados
gerados em diretório temporário. O teste de FDS confere a cor desenhada
pelo firmware sintético e o aviso quando `disksys.rom` está ausente.
O teste de PC Engine CD confere a recusa sem firmware, a abertura de
`.cue` com trilha separada e a integridade de `syscard3.pce` após o
encerramento. Ambos validam vídeo, áudio e restauração de estado.
A matriz existente do CI inclui Windows e Linux. [Fumaça do catálogo](../../.github/workflows/catalog-smoke.yml)

No Windows x64, o exemplo FDS de Brad Smith rodou avulso e em `.zip`
com `disksys.rom` original. O exemplo Super CD-ROM² do HuC mostrou
`HELLO, WORLD` depois de receber Start com `syscard3.pce` original.
O core-host confirmou vídeo, saída PCM e restauração de save state.
Sem a BIOS de FDS, o core aceitava o disco e exibia uma tela verde
porque preenchia a ROM de BIOS com bytes zerados. O ReEmu agora pede
o arquivo de 8 KiB ao abrir `.fds`, inclusive dentro de arquivo
compactado. [Exemplo FDS](https://github.com/bbbradsmith/NES-ca65-example/tree/fds) ·
[Exemplo HuC](https://github.com/pce-devel/huc/tree/master/examples/asm/elmer/scd-bios-hello) ·
[Carregador FDS do MesenCE](https://github.com/libretro/MesenCE/blob/master/Core/NES/Mappers/FDS/Fds.cpp)

O Mesen2 usa a revisão Super Game Boy 2. Com `SGB2.sfc` na pasta de BIOS,
o carregamento normal do ReEmu abriu o cartucho compatível e produziu
quadros de 256 × 239. Com apenas `SGB1.sfc`, o carregamento comum abriu
como Game Boy, em 160 × 144. A tentativa isolada com `SGB1.sfc` pelo
subsistema de dois arquivos terminou com uma violação de acesso dentro
da DLL. O Mesen-S aceita `SGB1.sfc` ao desligar **Use SGB2** nas opções
do core. [Configuração padrão no MesenCE](https://github.com/libretro/MesenCE/blob/master/Core/Shared/SettingTypes.h) · [Carregamento de firmware no MesenCE](https://github.com/libretro/MesenCE/blob/master/Core/SNES/BaseCartridge.cpp) · [Opções do Mesen-S](https://docs.libretro.com/library/mesen-s/)

No Windows x64, Donkey Kong e Kirby's Dream Land 2 rodaram em `.zip` por
600 quadros a 256 × 239 com áudio. Os testes usaram `SGB2.sfc` no Mesen2
e as duas revisões no Mesen-S, com hashes conferidos no catálogo oficial.
Os dois jogos reproduziram os mesmos quadros após restaurar o save state.
Em Donkey Kong, acionar Start a partir do mesmo estado alterou a sequência
de quadros nas três combinações. A contagem de amostras de áudio após a
restauração variou em relação à execução contínua. [Hashes dos firmwares](https://github.com/libretro/libretro-database/blob/master/dat/System.dat)

A fumaça dos dois cores abre um cartucho GB com a marcação SGB e
`SGB2.sfc` sintético. Ela verifica a resolução de 256 × 239, a cor
produzida pelo firmware, áudio, restauração de save state e integridade
do firmware após a execução. [Fumaça do catálogo](../../.github/workflows/catalog-smoke.yml)

A sondagem isolada do Mesen 2 no Windows travava ao descarregar a DLL
após `retro_init` sem conteúdo. O modo `--probe` agora reutiliza a saída
de processo já usada pelo modo `--run` no Windows, com a DLL carregada
até o encerramento. [Inicialização e encerramento do core](https://github.com/libretro/MesenCE/blob/master/Libretro/libretro.cpp)

A ROM de fumaça de SNES agora inicializa os registradores de vídeo que
afetavam a cor de fundo, além de desligar HDMA e interrupções. Antes do
ajuste, a imagem do bsnes-jg aparecia preta de forma intermitente. Com a
ROM atualizada, as 12 aberturas independentes no Windows produziram
180 quadros com imagem, áudio e restauração de save state. [ROM de teste](../../crates/core-catalog/src/smoke_roms.rs#L119)

A auditoria inclui os cores já catalogados. Usar a lista atual como fonte de IDs
e registrar resultado por plataforma e versão, com teste de conteúdo e
recursos além do probe. [Catálogo do ReEmu](../../crates/core-catalog/src/catalog.rs)
