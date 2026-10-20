# 15 — Ports nativos: recomp e decomp de jogos (launcher no ReEmu)

> **Status: backlog (`todo` de avaliação, TASKS.md › Funcionalidades).**
> Este documento é a pesquisa de base (2026-10-17 a 2026-10-20) para quando
> o ReEmu for abrir ports nativos de jogos — programas que rodam o jogo sem
> emulador — a partir da biblioteca. Nada aqui foi implementado ainda.
> Licenças e números conferidos na API do GitHub em **2026-10-20**; confira
> de novo antes de implementar (os projetos mudam rápido).

## 1. O que são

| Tipo | Como é feito | Exemplos |
|---|---|---|
| **Decomp** (decompilação) | Fãs reescrevem à mão, em C/C++, o código do jogo até ele compilar **idêntico** à ROM original (*matching decomp*). Depois alguém faz um **port** para PC trocando a camada de hardware por SDL/OpenGL/Vulkan. | zeldaret/oot → Ship of Harkinian; n64decomp/sm64 → sm64ex; zeldaret/tp → Dusklight; pret (Pokémon) |
| **Recomp** (recompilação estática) | Uma ferramenta lê o código de máquina da ROM e o traduz **automaticamente** para C/C++ (antes de rodar, não em tempo real como um JIT). Esse código é compilado junto com um **runtime** que faz o papel do console (memória, SO, áudio, GPU). | N64Recomp → Zelda 64: Recompiled; XenonRecomp → Unleashed Recompiled; cps2recomp |
| **Reimplementação** | O motor do jogo é refeito do zero (ou a partir da linguagem original), e só os dados vêm do jogo. | OpenGOAL (Jak and Daxter), zelda3 (A Link to the Past), DevilutionX (Diablo) |

Em todos os casos o projeto **não distribui os assets** (gráficos, sons,
níveis): o usuário fornece a própria ROM/disco, e o port extrai o que
precisa dela. Recomp não precisa de decomp pronta — é por isso que essa via
cresceu tanto em 2024–2026.

## 2. Como uma recompilação estática funciona

Resumo do pipeline (N64Recomp, cps2recomp e XenonRecomp seguem a mesma
ideia):

1. **Entrada:** o binário do jogo + metadados. No N64Recomp, um TOML com
   caminhos, stubs, funções puladas e patches, e os símbolos vêm de um **ELF**
   ("the only way to provide the required metadata is by passing an elf
   file"). Projetos sem símbolos descobrem o código lendo o programa e
   **jogando sozinhos** com entradas inventadas (`make explore` do cps2recomp,
   ~10 min).
2. **Tradução instrução a instrução, bem literal:** `addiu` vira
   `ctx->r4 = ADD32(ctx->r4, 0X20);`, `jal` vira chamada de função, e os
   *delay slots* do MIPS são resolvidos duplicando a instrução.
3. **Fluxo indireto (o ponto difícil):**
   - `jalr` (ponteiro de função/virtual) vira `LOOKUP_FUNC(ctx->r25)(rdram, ctx)`,
     resolvido pelo runtime numa tabela;
   - `jr` usado com *jump table* vira `switch`, quando o padrão é reconhecido
     (no Xbox 360 os padrões variam por compilador e por jogo);
   - código não descoberto cai num **interpretador**, e o projeto conta essas
     ocorrências para levá-las a zero (cps2recomp);
   - código automodificável é raro nesses alvos e não é tratado de forma
     geral (não verificado para o N64Recomp).
4. **Overlays e relocações:** o N64Recomp trata overlays estáticos e
   relocáveis (`RELOC_HI16`/`RELOC_LO16` resolvidos pelo runtime). Relocação
   de código mapeado por TLB ainda não existe.
5. **Coprocessadores:** a microcode do RSP do N64 também pode ser
   recompilada (sem overlays de RSP). No CPS-2, o Z80 de som é recompilado ou
   trocado por C++ escrito à mão.
6. **Runtime:** memória, interrupções, temporização, áudio e vídeo. No N64 o
   runtime é o **N64ModernRuntime (GPL-3.0)** e o vídeo é o **RT64 (MIT)**; no
   CPS-2 é uma "máquina" própria sobre a API de GPU do SDL 3 (Metal/Vulkan/D3D12).
7. **Verificação:** o código traduzido é comparado com o interpretador, e
   gravações de partida precisam gerar os mesmos hashes quadro a quadro
   (cps2recomp). Trechos reescritos à mão, como o modo treino, são conferidos
   chamada a chamada contra o original.
8. **Extras:** como o jogo vira código nativo, os ports adicionam
   widescreen, alta taxa de quadros, mods, gyro, run-ahead, modo treino e
   trilhas sonoras próprias.

Limites do N64Recomp listados no próprio README: testado sobretudo com
compiladores antigos (gcc 2.7.2, IDO) e clang (o mips gcc moderno "may trip
up the recompiler"); hooks de patch ainda não implementados; documentação de
opções incompleta.

## 3. Catálogo e licenças (GitHub, 2026-10-20)

`sem` = o GitHub não reconheceu nenhuma licença. Sem licença vale "todos os
direitos reservados": **não copiar código**. Abrir o programa oficial à
parte, como processo separado, continua possível.

### Ferramentas e runtimes

| Projeto | O que é | Licença | ★ | Último push |
|---|---|---|---|---|
| N64Recomp/N64Recomp | recompilador de N64 | **MIT** | 8277 | 2026-05-27 |
| N64Recomp/N64ModernRuntime | runtime dos ports de N64 | GPL-3.0 | 181 | 2026-09-29 |
| rt64/rt64 | renderizador de N64 (D3D12/Vulkan/Metal) | **MIT** | 1618 | 2026-10-20 |
| hedge-dev/XenonRecomp | recompilador de Xbox 360 (PowerPC → C++, só x86) | **MIT** | 6578 | 2025-08-04 |
| hedge-dev/XenosRecomp | shaders de Xbox 360 → HLSL | **MIT** | 504 | 2026-03-12 |
| rexglue/rexglue-sdk | runtime/SDK de Xbox 360, derivado do Xenia | própria (não reconhecida) | 1133 | 2026-10-02 |
| ran-j/PS2Recomp | recompilador de PS2 (só CPU; GS/VU1/DMA ainda stubs) | GPL-3.0 | 3611 | 2026-10-14 |
| tmcleroy/cps2recomp | kit + 5 ports de CPS-2 | GPL-3.0 | 54 | 2026-10-20 |
| tmcleroy/satrecomp | kit de Saturn (início) | GPL-3.0 | 1 | 2026-10-17 |
| tmcleroy/recomp-common | menu e launcher compartilhados | GPL-3.0 | 0 | 2026-10-19 |
| wizardengineer/psprecomp | PSP: análise em Rust, gera C++ | GPL-2.0 | 16 | 2026-10-02 |
| Kenix3/libultraship | base dos ports da Harbour Masters | **MIT** | 363 | 2026-10-12 |

### Ports jogáveis

| Projeto | Jogo / sistema | Licença | ★ | Observação |
|---|---|---|---|---|
| Zelda64Recomp/Zelda64Recomp | Majora's Mask (N64) | GPL-3.0 | 7482 | aceita só a ROM US; Win/Linux/macOS; mods |
| BanjoRecomp/BanjoRecomp | Banjo-Kazooie (N64) | GPL-3.0 | 2063 | |
| sonicdcer/Starfox64Recomp | Star Fox 64 (N64) | GPL-3.0 | 391 | |
| HarbourMasters/Shipwright | Ocarina of Time (decomp) | sem | 5690 | Ship of Harkinian |
| HarbourMasters/2ship2harkinian | Majora's Mask (decomp) | CC0 | 2349 | |
| HarbourMasters/Starship | Star Fox 64 (decomp) | CC0 | 2107 | |
| HarbourMasters/SpaghettiKart | Mario Kart 64 (decomp) | sem | 2431 | |
| fgsfdsfgs/perfect_dark | Perfect Dark (decomp) | **MIT** | 2066 | |
| sm64pc/sm64ex | Super Mario 64 (decomp) | sem | 1869 | sem builds oficiais |
| hedge-dev/UnleashedRecomp | Sonic Unleashed (Xbox 360) | GPL-3.0 | 5230 | instala a partir do dump do usuário |
| open-goal/jak-project | Jak and Daxter (PS2, reimplementação) | **ISC** | 3595 | só x86-64; tem launcher próprio |
| diasurgical/devilutionX | Diablo (reimplementação) | própria | 9909 | precisa do DIABDAT.MPQ |
| snesrev/zelda3 | A Link to the Past (SNES, reimplementação) | própria | 4883 | precisa da ROM |
| Rubberduckycooly/RSDKv5-Decompilation | Sonic Mania / RSDK | própria | 569 | |

Outros ports citados pelos agregadores (conferir repositório e licença antes
de usar): Mario Kart 64 Recomp, Donkey Kong 64 (Rekongpiled), Bomberman 64,
Harvest Moon 64, Dr. Mario 64, Dinosaur Planet, GoldenEye, Snowboard Kids 2,
Extreme-G (N64); Dusklight (Twilight Princess, GameCube); Mario Kart Wii e
Super Mario Galaxy "Nebula" (Wii); Lost Odyssey, Blue Dragon, Deadly
Premonition (Xbox 360); Syphon Filter 2, Street Fighter Alpha 3 (PS1).
Alguns agregadores marcam ports como feitos com IA — tratar com mais cautela.

Diretórios mantidos pela comunidade, bons para acompanhar: Recompedia,
Recompendium, recompsoftware.com, decomps.net, heldgames.com, gamerbrowser.com,
readonlymemo.com (lista mensal).

## 4. Licença e direito autoral — as regras para o ReEmu

1. **O ReEmu é MIT.** Traduzir para Rust (ou copiar) código **GPL** obriga o
   ReEmu a virar GPL; código **sem licença** não pode ser copiado. É a mesma
   regra do doc 14 (emuladores nativos).
2. **Executar um programa GPL como processo separado não contamina o ReEmu.**
   É como já fazemos com cores libretro baixados.
3. **Código de jogo é obra protegida.** Mesmo decomps CC0/MIT reproduzem a
   lógica de jogos com direitos autorais. O ReEmu **nunca** guarda, gera ou
   distribui código traduzido ou assets de jogo.
4. **Como as empresas agiram até agora (padrão prático, não regra legal):**
   - as notificações miram **binários prontos com assets** e redes que
     distribuem chaves, não o código-fonte das decomps;
   - a Nintendo já agiu contra mods feitos sobre a decomp do SM64;
   - em 2026, Activision e Bethesda derrubaram decompilações "vibe-coded",
     reconstruídas com IA;
   - fair use em engenharia reversa depende do país e do método.
5. **Downloads:** se o ReEmu oferecer baixar um port, só a **release oficial**
   do repositório do projeto, nunca uma build de terceiros, e só depois de
   confirmar que a release **não traz assets do jogo**. Alguns projetos (o
   Zelda64Recomp) avisam que "the original game is required to build or run".

## 5. E em Rust?

Não existe hoje um recompilador estático maduro em Rust. Para o ReEmu, o que
dá para aproveitar sem problema de licença:

| Projeto | Licença | Serve para |
|---|---|---|
| Decompollaborate/rabbitizer | MIT (C, com bindings Rust; crate `rabbitizer` no-std) | decodificar MIPS (N64, PS1, PS2, PSP), inclusive em tempo de compilação |
| crate `spimdisasm` (Rust) | conferir | disassembler MIPS I–IV com instruções do RSP |
| encounter/decomp-toolkit | Apache-2.0 (Rust) | toolkit de decomp de GameCube/Wii (PowerPC): análise e splitting de DOL/REL |
| encounter/objdiff | Apache-2.0 (Rust) | diff de objeto para decomp (MIPS via rabbitizer, PPC, SuperH) |
| ppc750cl (Rust) | conferir | decodificar o PowerPC 750CL (GameCube/Wii) |
| N64Recomp, XenonRecomp, RT64 | MIT (C++) | referência de projeto; o RT64 pode ser usado via FFI |
| psprecomp | GPL-2.0 | só como referência de comportamento (não copiar) |
| GCRecomp (GameCube → Rust) | conferir | experimento que gera **Rust** em vez de C |

**Caminhos possíveis, em ordem de custo:**

- **A — Launcher (recomendado como 1º passo).** O ReEmu não compila nada:
  cadastra e abre os ports oficiais. É barato, sem risco de licença, e já dá
  ao usuário uma entrada única na biblioteca. Detalhes na seção 6.
- **B — Recompilador próprio em Rust (longo prazo, só se valer a pena).**
  - **Decodificação:** rabbitizer (MIPS) ou ppc750cl/decomp-toolkit (PPC).
  - **Análise:** funções, jump tables, overlays e fallback para interpretador.
  - **Geração:** código Rust (o `build.rs` gera módulos) ou IR para Cranelift (Apache-2.0, AOT).
  - **Runtime nosso:** wgpu para vídeo (o que o ReEmu já tem), áudio e input do ReEmu.
  - **Custo:** é trabalho por jogo, porque cada recomp exige metadados,
    patches e ajustes manuais.
  - **Licença:** o runtime não pode ser derivado do N64ModernRuntime (GPL);
    teria de ser escrito do zero, a partir de documentação de hardware e de
    testes contra o comportamento, como no doc 14.
  - **Ordem:** só faz sentido depois do doc 14 e de ter o Caminho A no ar.

## 6. Proposta de arquitetura do launcher (Caminho A)

**Modelo (domain):** um `NativePort` no catálogo, mantido no repositório como
JSON (como o catálogo de cores):

```text
id, nome, systemId, tipo (recomp | decomp | reimplementação), licença
casamento com a biblioteca: nome de set MAME (cps2recomp: sfa2u, spf2t,
  vsavj, vsav2, vhunt2) | hash da ROM (sha1/md5/crc) | serial do disco
fonte: github owner/repo + padrão do asset por SO/arquitetura
executável relativo por SO, modelo de argumentos (ex. "--rom {rom}"),
  modo portátil (Zelda64Recomp: portable.txt), pastas de save/config
```

**Fluxo:**

1. **Associar:** o jogo da biblioteca (já identificado por hash ou set
   MAME) ganha a opção "Port nativo" se houver uma entrada que case.
2. **Instalar:** o usuário aponta a pasta de um port já baixado, ou o
   ReEmu baixa a release oficial pela API de releases do GitHub (asset por
   SO), para `<dados>/ports/<id>/<versão>`. Confere o digest do asset e
   mostra a licença.
3. **Abrir:** o botão Jogar oferece "Port nativo" ao lado do core.
   - O ReEmu inicia o processo (`std::process::Command`) com a ROM ou o
     caminho que o port pede.
   - Acompanha o tempo de jogo e volta ao frontend quando o processo fecha.
   - O cps2recomp aceita `cps2recomp-launcher --play sfa2` e
     `sfa2 --rom caminho/sfa2u.zip`. O Zelda64Recomp não documenta argumentos.
4. **Atualizar:** checar novas releases, como o app já faz com cores e bezels.

**Pontos de atenção:**

- **Controle:** os ports usam SDL. O ReEmu (gilrs) precisa largar o
  controle ou ficar só ouvindo enquanto o port roda, senão a navegação do
  menu reage por baixo.
- **Sem overlay/pausa do ReEmu:** a janela é do port, então save states,
  shaders e conquistas são os do próprio port. O RetroAchievements depende de
  integração de cada port.
- **Validação da ROM:** cada port aceita uma versão específica (Zelda64Recomp:
  só a US). O ReEmu deve avisar antes de abrir se o hash não bate.
- **Plataformas:** ports só x86-64 (XenonRecomp, OpenGOAL), macOS sem
  notarização, Flatpak no Linux, SmartScreen no Windows.
- **Prior art:** o ReComp Launcher (TheGh0stShip) já faz download, update e
  tempo de jogo para recomps da Nintendo, e o Harbour lista ports como o
  Shipwright. Vale olhar a experiência deles; não copiar código sem
  conferir a licença.

**Primeiros candidatos** (licença boa e builds oficiais; conferir antes):
cps2recomp (CLI documentada, sets MAME que a biblioteca já usa),
Zelda64Recomp, 2Ship2Harkinian, Shipwright, Starship, perfect_dark, OpenGOAL.

## 7. Perguntas em aberto

- O catálogo de ports fica no repo, atualizado à mão, ou vem de um índice
  externo (Recompedia)? Índice externo traz risco de links para builds de
  terceiros.
- Mostrar ports com licença `sem` (Shipwright, SpaghettiKart)? Abrir é
  legal; só não podemos redistribuir.
- Ports que pedem extração de assets na 1ª execução: o ReEmu só aponta a
  ROM, ou também chama o extrator do port?
- Tempo de jogo e "jogado recentemente" de um port contam no mesmo jogo da
  biblioteca (mesmo `romId`)?

## 8. Fontes

- N64Recomp (README, licença): https://github.com/N64Recomp/N64Recomp
- Zelda64Recomp (ROM US, saves, portable.txt, sem assets): https://github.com/Zelda64Recomp/Zelda64Recomp
- cps2recomp (README e docs/TOOLKIT.md): https://github.com/tmcleroy/cps2recomp
- XenonRecomp README: https://cdn.jsdelivr.net/gh/hedge-dev/XenonRecomp@main/README.md
- PS2Recomp (Time Extension, 2026-01): https://www.timeextension.com/news/2026/01/native-pc-ports-of-ps2-games-could-be-on-the-way-thanks-to-new-recompilation-experiment
- Listas e diretórios: https://readonlymemo.com/decompilation-projects-and-n64-recompiled-list/ · https://heldgames.com/guides/recompiled-games-list · https://heldgames.com/guides/n64-recompiled-games-list · https://heldgames.com/guides/retro-decompilation-recompilation-explained · https://recompedia.com/ · https://recompendium.com/en/ · https://recompsoftware.com/recompiled-games-list/ · https://decomps.net/?sort=new · https://gamerbrowser.com/pc-ports · https://amiibodoctor.com/2026/10/15/all-decompilation-pc-ports-of-retro-games-listed/ · https://retrolunch.com/blog/posts/2026-03-31-pc-recomps.html
- Fluxo indireto e fallback (outros recompiladores): https://github.com/sp00nznet/xboxrecomp/pull/186 · https://github.com/melikootje/pokesunrecomp/issues/7
- Legal e fiscalização: https://www.thegamer.com/ai-vibecoded-decompilations-takedown-fallout-mw2-dmca/ · https://aisignal.day/en/ai-game-decompilation-2026/ · https://www.videogameschronicle.com/news/nintendo-issues-dmca-takedown-on-more-switch-emulators-but-at-least-one-is-fighting-back/ · https://kotaku.com/the-vibe-coders-have-come-for-nintendo-and-obviously-this-wont-end-well-2000745258 · https://www.retroreversing.com/source-code/decompiled-retail-console-games · https://en.wikipedia.org/wiki/Ship_of_Harkinian
- Rust: https://github.com/Decompollaborate/rabbitizer · https://lib.rs/crates/rabbitizer · https://crates.io/crates/spimdisasm · https://crates.io/crates/objdiff-core · https://github.com/encounter/decomp-toolkit · https://github.com/wizardengineer/psprecomp · https://tomevault.io/tome/KaiserGranatapfel/GameCubeRecompiled
- Launchers existentes: https://github.com/TheGh0stShip/ReCompLauncher
- Licenças e números: API do GitHub (`gh api repos/<owner>/<repo>`), 2026-10-20.
