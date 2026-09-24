# 14 — Emuladores nativos em Rust (substituir cores libretro)

> **Status: `blocked`.** Só começa depois que o ReEmu for **totalmente
> compatível com cores libretro** (critérios na Fase 0). Até lá, este
> documento é o plano — nada aqui deve ser implementado antes.

## Objetivo

Recriar em Rust, dentro do ReEmu, os emuladores dos sistemas que ele cobre,
**substituindo aos poucos** os cores libretro: cada sistema ganha um core
nativo que passa a ser o padrão quando atinge a qualidade do core libretro
que ele substitui. Os cores libretro continuam disponíveis como alternativa
— nunca são removidos por este projeto.

Por quê: um core nativo roda no mesmo processo/arquitetura do app (sem FFI
C, sem o contrato libretro no meio), dá pra testar e depurar com as
ferramentas do Rust, e porta pro Android (etapa 11) sem depender do build
de cada core para cada ABI.

## A regra que decide tudo: licença

O ReEmu é **MIT**. Traduzir o código de um emulador para Rust é criar uma
**obra derivada** dele — o resultado herda a licença do original. Então:

| Licença do original | Pode portar pro ReEmu? |
|---|---|
| **Permissiva** (MIT/Expat, BSD, ISC, Apache-2.0) | **Sim.** Manter o aviso de copyright do original nos arquivos portados e num `NOTICE`. |
| **MPL-2.0** | Sim, com cuidado: é copyleft **por arquivo** — os arquivos Rust traduzidos de arquivos MPL continuam MPL-2.0 (cabeçalho próprio); o resto do ReEmu segue MIT. |
| **GPL** (2 ou 3) | **Não traduzir.** Obrigaria o ReEmu inteiro a virar GPL. Pode ser usado como *comportamento de referência* (rodar e comparar saída), não como código-fonte a ler e converter. |
| **Não comercial** / sem derivados | **Não.** Nem como base. |

Fontes de verdade pro comportamento do hardware, sem problema de licença:
documentação técnica pública (wikis e manuais de hardware), **testes
diferenciais contra o próprio core libretro** (o ReEmu já sabe rodar os
dois — ver Fase 1) e o código das fontes permissivas abaixo.

## Levantamento (2026-09-24)

Emuladores **standalone** (fora do RetroArch — alguns também têm port
libretro, o que não muda a licença). Licença conferida no próprio
repositório (API do GitHub + texto do arquivo de licença), não de memória.

### Livres e permissivos — base do projeto

| Emulador | Sistemas | Licença | Linguagem |
|---|---|---|---|
| **ares** (`ares-emulator/ares`) | Atari 2600 e 5200, ColecoVision, NES, Game Boy/Color, GBA, Mega Drive, Master System/Game Gear, SG-1000, MSX, My Vision, N64, Neo Geo, Neo Geo Pocket, PC Engine, PlayStation, Saturn, SNES, ZX Spectrum, WonderSwan (pastas `a26 a52 cv fc gb gba md ms msx myvision n64 ng ngp pce ps1 saturn sfc sg spec ws` do repositório) | **ISC** | C++ |
| **SameBoy** (`LIJI32/SameBoy`) | Game Boy / Color | **Expat (MIT)** — exceto as pastas iOS/HexFiend | C |
| **SkyEmu** (`skylersaleh/SkyEmu`) | Game Boy/Color, GBA, Nintendo DS | **MIT** | C |
| **rboy** (`mvdnes/rboy`) | Game Boy / Color | **MIT** | **Rust** |
| **TetaNES** (`lukexor/tetanes`) | NES | **Apache-2.0** | **Rust** |
| **Play!** (`jpd002/Play-`) | PlayStation 2 | **BSD** | C++ |
| **mGBA** (`mgba-emu/mgba`) | GBA, Game Boy | **MPL-2.0** | C |
| **vAmiga** (`dirkwhoffmann/vAmiga`) | Amiga | núcleo **MPL-2.0**, CPU 68000 (Moira) **MIT**; o app é GPL-3 | C++ |
| **VirtualC64** (`dirkwhoffmann/VirtualC64`) | Commodore 64 | núcleo **MPL-2.0**, CPU (Peddle) **MIT**; reSID e o app são GPL | C++ |
| **Cemu** (`cemu-project/Cemu`) | Wii U | **MPL-2.0** | C++ |

O **ares sozinho cobre quase todos os sistemas do ReEmu** com licença ISC —
é a espinha dorsal do plano.

### Livres, mas GPL — só como referência de comportamento

Mesen2, puNES, Nestopia (NES) · Gearboy, Gearsystem, Geargrafx, Gearcoleco,
Gearlynx · mooneye-gb (Rust, GPL-3) · NanoBoyAdvance (GBA) · bsnes (SNES) ·
mupen64plus, simple64, gopher64 (N64; gopher64 é Rust, GPL-3) · pcsx-redux,
rustation (PS1; rustation é Rust) · PCSX2 (PS2) · PPSSPP (PSP) · melonDS,
DeSmuME, NooDS (DS) · Azahar (3DS) · Dolphin (GameCube/Wii) · Flycast
(Dreamcast) · Ymir (Saturn) · Stella (Atari 2600) · FS-UAE (Amiga) ·
openMSX (MSX) · MAME (GPL no conjunto; muitos arquivos são BSD-3 —
aproveitável **arquivo por arquivo**, conferindo o cabeçalho) · DOSBox-X,
DOSBox Staging (DOS) · ScummVM · Mednafen.

### Não livres — fora

Snes9x e FBNeo (proibido uso comercial), Genesis Plus GX (idem — "não pode
ser vendido nem usado em produto ou atividade comercial"), DuckStation
(CC BY-NC-ND 4.0: não comercial e **sem obras derivadas**).

## Ordem de conversão (do mais fácil pro mais difícil)

Critérios, em ordem de peso: (1) hardware simples; (2) existe fonte
**permissiva** de alta qualidade; (3) reaproveita chips já feitos num passo
anterior; (4) existe implementação em Rust pra adaptar em vez de escrever;
(5) existem ROMs de teste conhecidas pra validar.

### Nível 1 — 8 bits (fundação dos chips)

| # | Sistema(s) | Fonte permissiva | Chips que nascem aqui | Observação |
|---|---|---|---|---|
| 1 | **ColecoVision + SG-1000** (+ My Vision) | ares `cv`, `sg`, `myvision` | **Z80**, **TMS9918A** (vídeo), **SN76489** (som) | O menor hardware da lista; os 3 chips são reusados em vários sistemas depois |
| 2 | **Master System / Game Gear** | ares `ms` | VDP Sega (derivado do TMS9918) | Reusa Z80 + SN76489 do passo 1 |
| 3 | **Game Boy / Color** | SameBoy (MIT), rboy (MIT, **Rust**), ares `gb` | SM83 | rboy é ponto de partida; SameBoy é a referência de precisão |
| 4 | **NES** | **TetaNES (Apache, Rust)**, ares `fc` | **6502**, PPU, APU, mappers | *Adaptar* o TetaNES à API de core nativo em vez de reescrever; o trabalho maior são os mappers |
| 5 | **Atari 2600** | ares `a26` | TIA, bankswitching | Reusa o 6502 do passo 4 (6507); o TIA exige timing por ciclo |
| 6 | **PC Engine / SuperGrafx** | ares `pce` | HuC6280 (65C02 estendido), VDC/VCE | Reusa a base 6502; CD fica pro nível 2 |
| 7 | **WonderSwan / Color** | ares `ws` | V30MZ | CPU nova, hardware pequeno |
| 8 | **Neo Geo Pocket / Color** | ares `ngp` | TLCS-900/H | Reusa Z80 (som) |
| 9 | **ZX Spectrum** e **MSX** | ares `spec`, `msx` | ULA, periféricos de fita/disco | Reusam Z80, TMS9918 (MSX), AY-3-8910 |
| 10 | **Atari 5200** | ares `a52` | ANTIC, GTIA, POKEY | Reusa 6502 |

### Nível 2 — 16 bits

| # | Sistema(s) | Fonte permissiva | Chips novos | Observação |
|---|---|---|---|---|
| 11 | **Mega Drive** | ares `md` | **68000**, VDP, YM2612 | 68000: referência adicional no Moira (MIT, do vAmiga) |
| 12 | **Game Boy Advance** | ares `gba`, SkyEmu (MIT), mGBA (MPL) | **ARM7TDMI** | ARM7 é reusado no DS |
| 13 | **SNES** | ares `sfc` | 65816, PPU, SPC700/DSP, coprocessadores (SuperFX, SA-1, DSP-n…) | Os coprocessadores são o grosso do trabalho |
| 14 | **Neo Geo AES/MVS** | ares `ng` | YM2610 | Reusa 68000 + Z80 |
| 15 | **Sega CD / 32X** e **PC Engine CD** | ares `md`, `pce` (conferir se o suporte está dentro dessas pastas antes de começar) | SH-2 (32X), leitor de CD | Extensões dos passos 6 e 11 |
| 16 | **Amiga** | vAmiga núcleo (MPL) + Moira (MIT) | Chips customizados (Agnus, Denise, Paula) | Arquivos portados do vAmiga ficam MPL-2.0 |
| 17 | **Commodore 64** | VirtualC64 núcleo (MPL) + Peddle (MIT) | VIC-II, **SID** | O reSID é GPL: o SID precisa ser implementado do zero a partir de documentação |

### Nível 3 — 3D / 32–64 bits

| # | Sistema | Fonte permissiva | Observação |
|---|---|---|---|
| 18 | **PlayStation** | ares `ps1` | R3000A, GPU (começar em rasterizador por software), SPU, CD-ROM |
| 19 | **Nintendo 64** | ares `n64` | VR4300, RSP, RDP — o RDP é o ponto crítico de desempenho (GPU) |
| 20 | **Saturn** | ares `saturn` | 2× SH-2 (reusa o do 32X), VDP1/VDP2, SCSP (68000) — avaliar a maturidade do núcleo `saturn` do ares antes de começar |

### Nível 4 — sem base permissiva completa (longo prazo)

| # | Sistema | Situação |
|---|---|---|
| 21 | **Nintendo DS** | SkyEmu (MIT) tem DS; reusa o ARM7 do GBA + ARM9 + motor 3D |
| 22 | **PlayStation 2** | Play! (BSD) é a única base permissiva; projeto enorme |

### Ficam com o core libretro (sem fonte livre e permissiva)

Dreamcast, PSP, GameCube/Wii, 3DS, DOS e ScummVM: todas as fontes são GPL.
Continuam com os cores libretro, a menos que o projeto decida mudar a
licença do ReEmu (decisão fora do escopo deste plano).

## Plano de ação

### Fase 0 — Portão: "totalmente compatível com libretro" (pré-requisito)

Este projeto só destrava quando **todos** estes itens do `TASKS.md`
estiverem `done`:

- [ ] Windows validado ponta a ponta (dev, instalador, jogos, chaveiro).
- [ ] Etapa 12: HW render Vulkan com os 2º/3º cores (Flycast, Mupen).
- [ ] Surface nativa de vídeo no Windows.
- [ ] **Teste de fumaça do catálogo**: um job que baixa cada core do
      catálogo e o carrega (`retro_load_game` com uma ROM de teste
      pública) em Linux e Windows — hoje só uma amostra foi carregada à mão.
- [ ] Save state, save RAM, input e áudio sem pendência aberta.

### Fase 1 — Fundação (antes do primeiro core)

1. **API de core nativo** — crate `crates/native-core-api` com um trait
   `NativeCore` no mesmo formato do que o `emu-session` já consome dos
   cores libretro (`load`, `run_frame` → frame + áudio, input, save
   state/RAM, `av_info`). O core nativo roda no mesmo processo filho
   (`reemu-core-host`) pelo mesmo IPC, então o resto do app não muda.
2. **Crate de chips** — `crates/emu-chips` pros componentes reusados entre
   sistemas (Z80, 6502, SN76489, TMS9918…), cada um testável sozinho.
3. **Harness de testes**:
   - **ROMs de teste de CPU/vídeo/som** por sistema (as suítes clássicas
     de cada comunidade). Conferir a licença de cada suíte antes de
     versionar no repositório; se não puder, o CI baixa na hora.
   - **Teste diferencial contra o core libretro** — a peça mais valiosa:
     roda a mesma ROM N quadros no core nativo e no libretro, com o mesmo
     input gravado, e compara hash de frame e de áudio quadro a quadro.
     O ReEmu já tem tudo pra rodar o lado libretro.
4. **Política de licença no CI** — `cargo deny` (ou equivalente) barrando
   dependência GPL/não comercial nos crates de core nativo, e um `NOTICE`
   com os avisos das fontes portadas.

### Fase 2 em diante — um sistema por vez, na ordem da tabela

Cada core passa pelos mesmos portões antes de virar o padrão do sistema:

1. **Portar** a partir da fonte permissiva (aviso de copyright mantido).
2. **ROMs de teste** do sistema passando.
3. **Diferencial**: bate com o core libretro num conjunto de jogos
   (quadros e áudio iguais, ou diferenças explicadas e aceitas).
4. **Desempenho**: dentro do orçamento de frame com folga, medido com
   `REEMU_PERF=1`.
5. **Save states** estáveis entre versões do core.
6. Vira o core **padrão** do sistema; o libretro continua selecionável.

Cada sistema é uma entrada própria no `TASKS.md` quando a fase começar.

## Riscos

- **Precisão**: cores libretro maduros têm anos de correções. O teste
  diferencial é o que evita regressão silenciosa.
- **Escopo**: os níveis 3 e 4 são projetos de anos. O plano entrega valor
  cedo (níveis 1–2 cobrem a maior parte da biblioteca típica) e pode parar
  em qualquer ponto sem prejuízo — o libretro continua lá.
- **Licença por arquivo**: MAME, vAmiga e VirtualC64 misturam licenças.
  Portar só arquivos cujo cabeçalho é permissivo e registrar a origem de
  cada arquivo portado.
