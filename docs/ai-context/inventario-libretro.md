# Inventário de compatibilidade libretro por core

**Recorte dos índices** — 2026-11-18, reconferido em 2026-11-23.
A disponibilidade indica presença no índice de pacotes. A compatibilidade
será comprovada pelo [roteiro de implementação](26-compatibilidade-libretro.md).

- **357 IDs** na união dos metadados de cores e dos dois índices.
- **261 IDs com pacote desktop**, dos quais 130 já estão no catálogo e 131 faltam. Entre os faltantes, 128 aparecem nos dois índices e 3 apenas no de Linux x64.
- **96 IDs adicionais** dos metadados oficiais ficam na fila de investigação de builds e alvos.

## Fontes do recorte

- Catálogo local na data do recorte. [Código do ReEmu](../../crates/core-catalog/src/catalog.rs)
- 350 arquivos de metadados de cores na revisão `eac3a2784d42`, excluído o modelo `00_example_libretro.info`. [Revisão do Core Info](https://github.com/libretro/libretro-core-info/tree/eac3a2784d428444aa2bd472039b5c5540141af1)
- Windows x64, 258 IDs. SHA-256 do índice `cee55ba24a8984fa8fae5f18da9f7f5882c2abdc369f1d538b14e0941dd49bf5`. [Índice do buildbot](https://buildbot.libretro.com/nightly/windows/x86_64/latest/.index-extended)
- Linux x64, 261 IDs. SHA-256 do índice `03369f12fd1bc819f3ff9ce03fc9a806bac0a77221827ef87a98d9be33da1952`. [Índice do buildbot](https://buildbot.libretro.com/nightly/linux/x86_64/latest/.index-extended)

## IDs publicados para desktop fora do catálogo (131)

Os exemplos de extensão vêm dos arquivos oficiais de informações.
Confirme o contrato do binário escolhido. Cada entrada passa pelos critérios
do roteiro antes de ser publicada como compatível.

### Pilotos e casos específicos (21)

- [ ] `azahar_libretro` — 3DS. Criar a integração 3DS e validar duas telas, toque e armazenamento. **Pacote:** Windows e Linux x64.
- [ ] `cemu_libretro` — Wii U. Validar WUA/RPX, GamePad e persistência de `mlc01` no fluxo Wii U. **Pacote:** Windows e Linux x64.
- [ ] `citra2018_libretro` — 3DS. Validar boot, layout, toque e saves da revisão 2018 como variante própria. **Pacote:** Windows e Linux x64.
- [ ] `citra_libretro` — 3DS. Validar as opções e o toque da DLL Citra após integrar a plataforma 3DS. **Pacote:** Windows e Linux x64.
- [ ] `dirksimple_libretro` — Laserdisc arcade game. Integrar `.ogv`/`.dirksimple` e testar reprodução e controle no fluxo LaserDisc. **Pacote:** Windows e Linux x64.
- [ ] `dolphin_libretro` — GameCube / Wii. Integrar GameCube em OpenGL com os arquivos `Sys` e validar Wii pelo [plano próprio](25-dolphin-libretro.md). **Pacote:** Windows e Linux x64.
- [ ] `DoubleCherryGB_libretro` — Game Boy/Game Boy Color. Validar Game Boy/Color e testar o modo de múltiplos jogadores quando anunciado. **Pacote:** Windows e Linux x64.
- [ ] `hatari2014_libretro` — Atari ST/STE/TT/Falcon. Validar disquetes e arquivos de sistema da variante 2014 separadamente. **Pacote:** Windows e Linux x64.
- [ ] `hatari_libretro` — Atari ST/STE/TT/Falcon. Validar TOS, disquete e teclado/joystick em Atari ST. **Pacote:** Windows e Linux x64.
- [ ] `hatarib_libretro` — Atari ST/STE/TT/Falcon. Validar TOS, seleção de máquina, disquete e troca de mídia no hatariB. **Pacote:** Windows e Linux x64.
- [ ] `hypseushw_libretro` — Laserdisc Arcade. Validar lançamento por framefile e acesso aos arquivos acompanhantes de áudio e vídeo. **Pacote:** Windows e Linux x64.
- [ ] `openmsx_libretro` — Metadados a confirmar. Confirmar os recursos pelo binário. Integrar cartucho, disco e arquivos de sistema do MSX. [openMSX](https://docs.libretro.com/library/openmsx/) **Pacote:** Windows e Linux x64.
- [ ] `panda3ds_libretro` — 3DS. Confirmar contexto gráfico do binário e testar layout e toque com a plataforma 3DS. **Pacote:** Windows e Linux x64.
- [ ] `parallel_dc_libretro` — Sega Dreamcast. Validar a variante Dreamcast separadamente do Flycast e registrar os backends selecionáveis. **Pacote:** Windows e Linux x64.
- [ ] `pcee2_libretro` — Sony PlayStation 2. Validar como alternativa de PS2 e conferir a negociação gráfica e os caminhos de firmware e saves. **Pacote:** Windows e Linux x64.
- [ ] `play_libretro` — Sony PlayStation 2. Testar Play! como alternativa para PS2 e confirmar o renderizador anunciado no binário. **Pacote:** Windows e Linux x64.
- [ ] `px68k_libretro` — Sharp X68000. Validar arquivos de sistema e disquetes da plataforma Sharp X68000. **Pacote:** Windows e Linux x64.
- [ ] `rpcs3_libretro` — PlayStation 3. Validar firmware, disco ou pasta PS3, Vulkan e persistência do armazenamento emulado. **Pacote:** Windows e Linux x64.
- [ ] `vita3k_libretro` — PlayStation Vita. Validar firmware e arquivos da versão. Testar VPK, instalação persistente e Vulkan. **Pacote:** Windows e Linux x64.
- [ ] `xemu_libretro` — Xbox. Validar boot, XISO e disco QCOW2, com persistência após fechar. **Pacote:** Windows e Linux x64.
- [ ] `xenia_edge_libretro` — Xbox 360. Validar ISO/XEX e saves em `content/` com Vulkan. Encaminhar D3D12 ao [roteiro de GPU](24-direct3d-por-core.md). **Pacote:** Windows e Linux x64.

### Alternativas para plataformas identificadas (8)

- [ ] `fbalpha2012_cps1_libretro` — CP System I. Testar a seleção do core no sistema já mapeado com `.zip`. **Pacote:** Windows e Linux x64.
- [ ] `fbalpha2012_cps2_libretro` — CP System II. Testar a seleção do core no sistema já mapeado com `.zip`. **Pacote:** Windows e Linux x64.
- [ ] `fbalpha2012_cps3_libretro` — CP System III. Testar a seleção do core no sistema já mapeado com `.zip`. **Pacote:** Windows e Linux x64.
- [ ] `fbalpha2012_neogeo_libretro` — Neo Geo. Testar a seleção do core no sistema já mapeado com `.zip`. **Pacote:** Windows e Linux x64.
- [ ] `fsuae_libretro` — Amiga. Testar a seleção do core no sistema já mapeado com `.adf`, `.ipf`, `.fs-uae`. **Pacote:** Linux x64.
- [ ] `geolith_libretro` — Neo Geo. Testar a seleção do core no sistema já mapeado com `.neo`, `.cue`, `.chd`. **Pacote:** Windows e Linux x64.
- [ ] `pcem_libretro` — PC. Testar a seleção do core no sistema já mapeado com `.exe`, `.com`, `.bat`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `rust_dos_libretro` — DOS. Testar a seleção do core no sistema já mapeado com `.exe`, `.com`, `.bat`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.

### Plataformas e formatos a confirmar (53)

- [ ] `81_libretro` — ZX81. Confirmar o mapeamento da plataforma e os formatos do core como `.p`, `.tzx`, `.t81`. **Pacote:** Windows e Linux x64.
- [ ] `amiarcadia_libretro` — Arcadia 2001. Confirmar o mapeamento da plataforma e os formatos do core como `.bin`, `.tvc`. **Pacote:** Windows e Linux x64.
- [ ] `applewin_libretro` — II. Confirmar o mapeamento da plataforma e os formatos do core como `.bin`, `.do`, `.dsk`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `ardens_libretro` — Arduboy. Confirmar o mapeamento da plataforma e os formatos do core como `.hex`, `.arduboy`. **Pacote:** Windows e Linux x64.
- [ ] `arduous_libretro` — Arduboy. Confirmar o mapeamento da plataforma e os formatos do core como `.hex`. **Pacote:** Windows e Linux x64.
- [ ] `b2_libretro` — BBC Micro. Confirmar o mapeamento da plataforma e os formatos do core como `.ssd`, `.dsd`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `bbkemu_libretro` — BBK Electronic Dictionary. Confirmar o mapeamento da plataforma e os formatos do core como `.gam`. **Pacote:** Windows e Linux x64.
- [ ] `bk_libretro` — BK-0010/BK-0011(M). Confirmar o mapeamento da plataforma e os formatos do core como `.bin`. **Pacote:** Windows e Linux x64.
- [ ] `coolbaremu_libretro` — Coolbar. Confirmar o mapeamento da plataforma e os formatos do core como `.cbe`. **Pacote:** Windows e Linux x64.
- [ ] `dingooemu_libretro` — Dingoo A320. Confirmar o mapeamento da plataforma e os formatos do core como `.app`, `.cc`, `.c2s`. **Pacote:** Windows e Linux x64.
- [ ] `emuscv_libretro` — Super Cassette Vision. Confirmar o mapeamento da plataforma e os formatos do core como `.cart`, `.bin`, `.rom`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `freechaf_libretro` — Channel F. Confirmar o mapeamento da plataforma e os formatos do core como `.bin`, `.chf`. **Pacote:** Windows e Linux x64.
- [ ] `galaksija_libretro` — Galaksija. Confirmar o mapeamento da plataforma e os formatos do core como `.gal`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `gam4980_libretro` — Longman 4980. Confirmar o mapeamento da plataforma e os formatos do core como `.gam`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `gametank_libretro` — GameTank. Confirmar o mapeamento da plataforma e os formatos do core como `.gtr`. **Pacote:** Windows e Linux x64.
- [ ] `geartowns_libretro` — FM Towns. Confirmar o mapeamento da plataforma e os formatos do core como `.cue`, `.chd`, `.iso`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `gw_libretro` — Handheld Electronic. Confirmar o mapeamento da plataforma e os formatos do core como `.mgw`. **Pacote:** Windows e Linux x64.
- [ ] `hyperscanemu_libretro` — HyperScan. Confirmar o mapeamento da plataforma e os formatos do core como `.bin`, `.cue`, `.zip`. **Pacote:** Windows e Linux x64.
- [ ] `jaxe_libretro` — CHIP-8. Confirmar o mapeamento da plataforma e os formatos do core como `.ch8`, `.sc8`, `.xo8`. **Pacote:** Windows e Linux x64.
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
- [ ] `sameduck_libretro` — Mega Duck. Confirmar o mapeamento da plataforma e os formatos do core como `.bin`, `.duck`. **Pacote:** Windows e Linux x64.
- [ ] `spmp8000emu_libretro` — SPMP8000. Confirmar o mapeamento da plataforma e os formatos do core como `.bin`. **Pacote:** Windows e Linux x64.
- [ ] `squirreljme_libretro` — Java ME. Confirmar o mapeamento da plataforma e os formatos do core como `.jar`, `.sqc`, `.jam`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `tamalibretro_libretro` — Tamagotchi P1. Confirmar o mapeamento da plataforma e os formatos do core como `.b`, `.bin`, `.rom`. **Pacote:** Windows e Linux x64.
- [ ] `theodore_libretro` — Thomson MO/TO. Confirmar o mapeamento da plataforma e os formatos do core como `.fd`, `.sap`, `.k7`. Testar abertura sem conteúdo. **Pacote:** Windows e Linux x64.
- [ ] `uzem_libretro` — Uzebox. Confirmar o mapeamento da plataforma e os formatos do core como `.uze`. **Pacote:** Windows e Linux x64.
- [ ] `vaporspec_libretro` — VaporSpec. Confirmar o mapeamento da plataforma e os formatos do core como `.vaporbin`. **Pacote:** Windows e Linux x64.
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

## IDs oficiais sem pacote nos índices Windows e Linux x64 (96)

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
- [ ] `daphne_libretro` — Arcade (various). Localizar pacote oficial utilizável e conferir o fluxo de framefile, ROM e vídeo da revisão.
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
- [ ] `open-source-notices` — Metadados a confirmar. Confirmar alvo, origem e disponibilidade do binário.
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
| `bsnes-jg_libretro` | Buildbot de 2026-11-23, versão 2.1.0. Descoberta e probe pelo loader. A fumaça do catálogo rodou 180 quadros com imagem, áudio e restauração de save state. Teste adicional confirmou o botão B do RetroPad e a restauração de 2 KiB de SRAM após reabertura. | Pacote publicado. Validação em Linux e dos subsistemas Super Game Boy, BS-X e Sufami Turbo pendente. |
| `mesen-s_libretro` | Buildbot de 2026-11-23, versão 0.4.0. O core-host rodou 180 quadros com imagem, áudio e restauração de save state para `.sfc`, `.gb` e `.gbc` com modo Color obrigatório. Testes adicionais confirmaram o botão B do RetroPad, a restauração da SRAM após reabertura, o modo Super Game Boy com as duas revisões de firmware e a inicialização de Satellaview com `.bs` avulso e em ZIP. | Pacote publicado. Execução em Linux e validação de jogos comerciais de Satellaview pendentes. [Documentação oficial](https://docs.libretro.com/library/mesen-s/) |
| `mesen2_libretro` | Buildbot de 2026-11-23, versão 2.0.0 anunciada pela DLL. O core-host rodou 180 quadros com imagem, áudio e restauração de save state em cada cartucho de teste de NES, SNES, GB, GBC, GBA, PC Engine, SuperGrafx, Master System, Game Gear, WonderSwan e WonderSwan Color. O GBA abriu sem BIOS com a opção de iniciar direto no cartucho aplicada pelo ReEmu. FDS e PC Engine CD passaram com mídia sintética e também com programas homebrew usando firmware original. Super Game Boy 2 passou com firmware sintético e original. | Pacote Linux verificado como ELF64 x86-64 com os símbolos Libretro. Execução em Linux e boot de discos comerciais pendentes. [Documentação oficial](https://docs.libretro.com/library/mesen2/) |

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
