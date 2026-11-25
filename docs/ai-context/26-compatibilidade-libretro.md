# 26 — Compatibilidade total com libretro

**Estado** — Backlog desde 2026-11-18.

**Meta** — Integrar todos os cores libretro, incluindo variantes, engines,
aplicativos, utilitários e cores que iniciam sem ROM. A compatibilidade é
aferida por **core, versão, sistema operacional, arquitetura e
renderizador**. A execução começa por Windows e Linux x64, os alvos
desktop atuais do ReEmu. A matriz se amplia com os outros alvos do app.
Entradas que dependem de build, plataforma ou correção upstream
continuam na fila.

## Ponto de partida

O [inventário por core](inventario-libretro.md) cruza o catálogo do ReEmu,
os metadados oficiais e os índices do buildbot. São **261 IDs
publicados para Windows ou Linux x64**, com **147 no catálogo** e
**114 faltando**. Há mais **95 IDs
oficiais** sem pacote nesses dois índices. Cada ID faltante tem uma
tarefa inicial e a situação do pacote. A disponibilidade de um binário
e a compatibilidade comprovada no ReEmu são verificações distintas.
[Catálogo atual](../../crates/core-catalog/src/catalog.rs) ·
[Core Info oficial](https://github.com/libretro/libretro-core-info) ·
[Buildbot Windows x64](https://buildbot.libretro.com/nightly/windows/x86_64/latest/) ·
[Buildbot Linux x64](https://buildbot.libretro.com/nightly/linux/x86_64/latest/)

Atualizar o inventário com as revisões dos índices e dos metadados.
Incluir ports mantidos fora do buildbot quando houver uma fonte
identificável. Manter entradas arquivadas, variantes e apelidos
confirmados para que remoções ou renomeações não aumentem a cobertura
por engano.

## Etapas de implementação

| Etapa | Tarefas pendentes | Resultado esperado |
| --- | --- | --- |
| **1. Inventário contínuo** | Corrigir a referência ao caminho antigo do catálogo em `scripts/gen_core_catalog.py`. Separar descoberta de binários da publicação no catálogo. Registrar fonte, alvo, versão, categoria, sistema e recursos. | Relatório de diferenças que inclui variantes, cores de um único alvo e lançamentos novos. |
| **2. Alternativas para plataformas conhecidas** | bsnes-jg, Mesen-S, Mesen 2 e DoubleCherryGB entraram no catálogo e passaram testes de cartucho no Windows. Conferir os recursos pendentes no inventário. A rodada em Linux está em [backlog](../../TASKS.md#testes-em-linux-x64). | Novas opções entram pelo fluxo normal, com regressões testadas nos cores padrão. |
| **3. Plataformas clássicas e mídia** | Hatari, Hatari 2014, hatariB e PX68k passaram o boot de disquete no Windows. O DirkSimple abriu vídeos LaserDisc com dados auxiliares, o HypseusHW abriu mídia pelo framefile Daphne e o openMSX abriu cartuchos com C-BIOS. Integrar os demais sistemas e formatos do inventário. | Importação confiável, saves persistentes e diagnóstico de arquivos de sistema ausentes. |
| **4. Engines, aplicativos e utilitários** | Usar a capacidade de iniciar sem conteúdo quando anunciada. Criar o fluxo de seleção de pasta ou arquivo para engines e mídias. Descobrir as dependências de dados e os diretórios graváveis por core. | Cada categoria tem forma de abrir, operar e encerrar pelo ReEmu. |
| **5. GameCube, Wii e 3DS** | Seguir o piloto Dolphin e usar Azahar para estabelecer formatos, layout e toque do 3DS. Validar Citra, Citra 2018, Citra Canary e Panda3DS individualmente. | GameCube e Wii ganham plataformas distintas, e o 3DS compartilha uma base com validação por core. [Plano Dolphin](25-dolphin-libretro.md) |
| **6. Wii U, PS3, Vita e Xbox** | Fazer pilotos independentes, cobrindo formatos, armazenamento emulado, GPU e controles. Liberar D3D12 do Xenia Edge após validar a base gráfica. | Importação, jogo e persistência aferidos por core e alvo. [Roteiro Direct3D](24-direct3d-por-core.md) |
| **7. Revalidação e alvos restantes** | Auditar os IDs já catalogados. Ampliar o smoke test para mais tipos de conteúdo e tratar as entradas sem pacote x64. Verificar outros alvos conforme o suporte do app e documentar impedimentos. | Matriz completa para o recorte, com próxima tarefa e evidência por combinação. |

O smoke test atual executa o probe dos cores e roda conteúdo em um
subconjunto de sistemas. Ampliar esse caminho reaproveitando o core-host,
as fixtures e os testes existentes. [Workflow](../../.github/workflows/catalog-smoke.yml) ·
[Fixtures](../../crates/core-catalog/src/smoke_roms.rs) ·
[Gerador do catálogo](../../scripts/gen_core_catalog.py)

## Tarefas específicas dos primeiros projetos

| Core | Trabalho próprio |
| --- | --- |
| **Dolphin** | Instalar `Sys`, validar GameCube em OpenGL com RetroPad e memória persistente, depois Wii com Classic Controller e Wiimote emulado. Fazer Vulkan e D3D11 em rodadas próprias. [Plano do Dolphin](25-dolphin-libretro.md) |
| **Azahar** | Validar OpenGL, os layouts de duas telas, o toque e a persistência. [Azahar](https://docs.libretro.com/library/azahar/) |
| **Citra, Citra 2018, Citra Canary e Panda3DS** | Reusar a integração 3DS e testar as opções, formatos, touch e saves de cada binário. Conferir a disponibilidade da variante Canary e o contexto gráfico específico do Panda3DS. [Citra](https://docs.libretro.com/library/citra/) · [Panda3DS](https://docs.libretro.com/library/panda3ds/) |
| **Cemu** | Importar WUA/RPX, validar GamePad e toque, persistir `mlc01` e comparar Vulkan e OpenGL. Exibir a limitação de save state prevista no port documentado. [Cemu](https://docs.libretro.com/library/cemu/) |
| **RPCS3** | Importar firmware, detectar disco e pasta PS3, avaliar a instalação de PKG e validar Vulkan, saves e encerramento após a criação de cache. [RPCS3](https://docs.libretro.com/library/rpcs3/) |
| **Vita3K** | Validar firmware e assets da versão, instalação por VPK, abertura pela biblioteca após instalar, Vulkan e saves. Avaliar OpenGL separadamente. [Vita3K](https://docs.libretro.com/library/vita3k/) |
| **xemu** | Validar XISO, arquivos de boot e disco QCOW2. Conferir controles, save no disco emulado e encerramento em OpenGL e Vulkan. [xemu](https://docs.libretro.com/library/xemu/) |
| **DirectXbox** | Confirmar um build e validar a inicialização experimental com Cromwell. Jogos comerciais dependem de evolução upstream. [DirectXbox](https://github.com/libretro/libretro-directxbox) |
| **Xenia Edge** | Identificar ISO/XEX, validar Vulkan, saves em `content/` e listas multidisco. D3D12 no Windows segue o projeto gráfico. [Xenia Edge](https://docs.libretro.com/library/xenia_edge/) |
| **Hatari, Hatari 2014 e hatariB** | Cada variante tem teste próprio de TOS, disquete, teclado/joystick e troca de disco quando anunciada. Compartilhar o perfil Atari ST. [Hatari](https://docs.libretro.com/library/hatari/) · [hatariB](https://docs.libretro.com/library/hatarib/) |
| **PX68k** | Sharp X68000 integrado com BIOS e boot de XDF, DIM, ZIP e M3U verificados no Windows. Validar HDF, gravação em disquetes e troca de disco pelo menu do ReEmu. Execução em Linux está em [backlog](../../TASKS.md#testes-em-linux-x64). [Evidência e formatos](inventario-libretro.md#sharp-x68000-no-windows) |
| **DirkSimple** | LaserDisc integrado com `.ogv` e `.dirksimple`, dados auxiliares automáticos e relógio por quadro. Validar vídeos completos dos jogos e executar a rodada Linux do backlog. [Evidência e formatos](inventario-libretro.md#laserdisc-com-dirksimple-no-windows) |
| **HypseusHW** | Framefile MPEG, ROM ZIP, vídeo e áudio integrados no Windows. Passaram OpenGL e CPU Fallback. Validar jogos completos, Vulkan, Singe e a rodada Linux. [Evidência e formatos](inventario-libretro.md#laserdisc-com-hypseushw-no-windows) |
| **Daphne** | Aguardando pacote x64. A compilação Windows com MinGW está bloqueada no backend de threads SDL. Depois de obter um binário, validar framefile, ROM e mídia. [Projeto oficial](https://github.com/libretro/daphne) |
| **openMSX** | Cartucho MSX2 avulso e em ZIP com C-BIOS passou no Windows. Validar boot de disquetes e fitas em máquinas reais, teclado físico e troca de disco. Execução em Linux está em [backlog](../../TASKS.md#testes-em-linux-x64). [Evidência e formatos](inventario-libretro.md#openmsx-no-windows) |
| **JAXE** | Plataforma CHIP-8 integrada com `.ch8`, `.sc8`, `.xo8` e `.hc8`. A fumaça de Windows usa uma ROM própria e a rodada Linux está em [backlog](../../TASKS.md#testes-em-linux-x64). [Documentação do JAXE](https://docs.libretro.com/library/jaxe/) |
| **Ardens** | Plataforma Arduboy integrada com `.arduboy` e `.hex` em pastas identificadas. No Windows, passaram leitura de flash FX empacotado, persistência da EEPROM e save state. Validar gravação no flash FX. A rodada Linux está em [backlog](../../TASKS.md#testes-em-linux-x64). [Documentação do Ardens](https://docs.libretro.com/library/ardens/) |
| **Arduous** | Alternativa para `.hex` na plataforma Arduboy. A validação no Windows cobriu vídeo, áudio, controle, save state e EEPROM separada do Ardens. A rodada Linux está em [backlog](../../TASKS.md#testes-em-linux-x64). [Código oficial](https://github.com/libretro/arduous) |
| **Uzem** | Plataforma Uzebox com cartuchos `.uze` avulsos ou compactados em ZIP e 7z. No Windows, passaram instalação, abertura dos três formatos e restauração de save state e EEPROM. O cartão SD emulado usa `system/uzem`. A rodada Linux está no [backlog](../../TASKS.md#testes-em-linux-x64). [Documentação oficial](https://docs.libretro.com/library/uzem/) |
| **VaporSpec** | Plataforma virtual com arquivos `.vaporbin` avulsos ou compactados em ZIP e 7z. No Windows, passaram instalação, abertura dos três formatos, botão B do RetroPad e restauração de save states. A versão testada produz vídeo de 256 × 192 e envia amostras silenciosas. A rodada Linux está no [backlog](../../TASKS.md#testes-em-linux-x64). [Documentação oficial](https://docs.libretro.com/library/vaporspec/) |
| **GameTank** | Cartuchos `.gtr` avulsos ou compactados em ZIP e 7z. No Windows, passaram vídeo, áudio, RetroPad, restauração de save states, persistência de flash `.xor` nos três formatos e de SRAM `.srm` em cartucho `SAVE`. A rodada Linux está no [backlog](../../TASKS.md#testes-em-linux-x64). [Documentação oficial](https://docs.libretro.com/library/gametank/) |
| **SameDuck** | Mega Duck / Cougar Boy com `.duck` e `.bin` avulsos ou compactados em ZIP e 7z. O scan de `.bin` usa o nome da pasta para identificar o console. No Windows, passaram instalação, vídeo, áudio, botão B do RetroPad e restauração de save states. A rodada Linux está no [backlog](../../TASKS.md#testes-em-linux-x64). [Documentação oficial](https://docs.libretro.com/library/sameduck/) |
| **GW** | Simuladores Handheld Electronic em `.mgw` avulso, ZIP e 7z. No Windows, passaram instalação, inicialização com a geometria do simulador, mapa de controles via Start e restauração da SRAM de 777 bytes. A rodada Linux está no [backlog](../../TASKS.md#testes-em-linux-x64). [Documentação oficial](https://docs.libretro.com/library/gw/) |
| **EightyOne** | Sinclair ZX81 integrado com identificação de `.p` pelo cabeçalho e seleção de core para `.tzx` pelo sistema do jogo. Confirmar T81 e teclado físico. Execução em Linux está em [backlog](../../TASKS.md#testes-em-linux-x64). [Documentação do EightyOne](https://docs.libretro.com/library/eightyone/) |

## Como concluir cada tarefa

1. **Disponibilidade.** Registrar origem, versão, licença, alvo e
   instalação. Para pacote ausente, verificar o alvo upstream ou um build
   reproduzível e registrar a evidência do impedimento.
2. **Integração.** Conferir documentação e código oficiais da revisão.
   Usar o que o core anuncia em `retro_get_system_info`, nos callbacks
   de ambiente e na negociação gráfica. Tratar formatos, caminho
   integral, VFS, arquivos auxiliares e opções conforme o contrato
   de cada core. [ABI libretro](https://github.com/libretro/RetroArch/blob/master/libretro-common/include/libretro.h)
3. **Validação.** Fazer probe, abrir pelo fluxo normal do app e testar
   saída, entrada, persistência quando disponível, encerramento e segunda
   abertura. Conferir tratamento de conteúdo inválido ou dependências
   ausentes quando aplicável. Usar fixture redistribuível quando houver.
4. **Publicação.** Marcar o que foi validado por alvo e versão. Apresentar
   ao usuário as capacidades do binário. A cobertura de boot e a de
   recursos são registradas separadamente.
5. **Regressão.** Acrescentar a combinação à fumaça ou à matriz local de
   GPU e revalidar quando a DLL mudar. Medir as rotas existentes e novas
   antes de mudar padrões. [Roteiro de desempenho](../perf/README.md)

Consultar fontes oficiais antes de decisões técnicas, citar a revisão
consultada e registrar os pontos sem evidência. Mudanças estruturais em
core ou GPU seguem o aviso prévio do projeto. [Instruções do ReEmu](../../CLAUDE.md) ·
[Plano unificado](19-plano-unificado.md)

**Acompanhamento da meta** — Mostrar quantas combinações foram validadas
em relação às identificadas por plataforma, com recursos testados e
impedimentos. A meta de 100% é aferida na matriz revisada por alvo.
A cobertura de jogos é acompanhada dentro de cada core.
