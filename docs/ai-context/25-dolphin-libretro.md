# 25 — Suporte ao core Dolphin

**Estado** — Backlog desde 2026-11-18. O objetivo é abrir jogos de GameCube
e Wii pelo fluxo normal do ReEmu com o core `dolphin_libretro`. A primeira
entrega valida GameCube com controle convencional e OpenGL. Wii entra em
uma etapa seguinte com controles emulados. Vulkan tem validação própria.
D3D11 depende do [projeto de Direct3D por core](24-direct3d-por-core.md).

## Base confirmada

- O Dolphin libretro documenta OpenGL, Vulkan e D3D11. O core requer os
  arquivos `dolphin-emu/Sys` no diretório de sistema. O perfil gravável
  fica sob `dolphin-emu/User` no diretório de saves. IPL de GameCube é
  opcional para a maior parte dos jogos e pode ser importado pelo usuário
  quando necessário. [Documentação do Dolphin libretro](https://docs.libretro.com/library/dolphin/)
- O arquivo de informações do core lista GameCube e Wii e declara extensões
  de discos, executáveis e listas. Conferir `retro_get_system_info`, opções
  de renderização e controles na DLL ou no SO exato antes de publicar
  formatos ou recursos no catálogo. [Arquivo oficial do core](https://github.com/libretro/libretro-super/blob/master/dist/info/dolphin_libretro.info) · [Código do port libretro](https://github.com/libretro/dolphin)
- O ReEmu já tem importação de imagens de disco, descoberta de cores locais,
  um processo filho para cores libretro e um fluxo de arquivos de sistema
  usado pelo PPSSPP. A instalação automática do Dolphin e as duas plataformas
  ainda precisam ser integradas. [Identificação de discos](../../crates/library-scan/src/disc_sniff.rs) · [Catálogo](../../crates/core-catalog/src/catalog.rs) · [Arquivos de sistema](../../apps/desktop/src-tauri/src/system_files.rs)
- O loader registra as portas anunciadas pelo core e hoje escolhe
  `RETRO_DEVICE_JOYPAD` para as portas. Antes de liberar os perfis do Wii,
  verificar os dispositivos e subtipos anunciados, selecionar os IDs por
  porta e levar a configuração ao processo filho. [Loader](../../crates/core-loader-desktop/src/loader.rs) · [API de controles](https://github.com/libretro/libretro-common/blob/master/include/libretro.h)

## Etapas

### A — Escolher o binário e validar a base

- [ ] Registrar a origem, versão e arquitetura do core Windows x64 e
      conferir a disponibilidade para Linux x64. Escolher um conjunto
      `Sys` compatível com a revisão do core, conferir origem e licença e
      validar instalação e detecção em um diretório limpo.
- [ ] Rodar um teste manual do core no processo filho com OpenGL. Confirmar
      negociação de contexto, caminho do conteúdo, vídeo, áudio, diretórios
      de sistema e saves e encerramento limpo. Registrar GPU, driver e DLL
      ou SO usado. Validar o caminho Windows/WGL e Linux/EGL separadamente
      antes de anunciar suporte em cada plataforma. [OpenGL no Dolphin](https://docs.libretro.com/library/dolphin/#opengl)

### B — Entrega inicial de GameCube

- [ ] Adicionar `dolphin_libretro` ao catálogo e associar o sistema
      `gamecube` ao core. Integrar nome, filtro e arte da plataforma no
      frontend. Incluir detecção de `Sys`, instalação por versão com
      validação do conteúdo e mensagem de recuperação. Tratar IPL como
      arquivo opcional importado pelo usuário.
- [ ] Começar com `.gcm`, `.iso` e `.rvz` quando confirmados no binário
      escolhido. Classificar ISO com a evidência do disco e da pasta,
      preservando a seleção manual quando o sistema for ambíguo. RVZ
      comporta GameCube e Wii. Validar a identificação pelo formato e
      usar pasta ou seleção manual até ter um leitor confiável.
      Evitar carregar imagens grandes inteiras na memória e testar regressões
      nas associações de ISO existentes.
      [Formatos do core](https://github.com/libretro/libretro-super/blob/master/dist/info/dolphin_libretro.info) · [Especificação do RVZ](https://github.com/dolphin-emu/dolphin/blob/master/docs/WiaAndRvz.md)
- [ ] Validar controle GameCube com RetroPad. Conferir botão A/B/X/Y,
      C-stick, gatilhos e mapeamento das portas. Testar cartão de memória
      e save state após fechar e reabrir o jogo. Usar diretórios de teste
      próprios para manter os dados reais separados.

### C — Acrescentar Wii

- [ ] Registrar `wii` como outra plataforma associada ao mesmo core.
      Validar ISO e RVZ para Wii e acrescentar WBFS se passar na DLL
      testada. Tratar WAD, listas multidisco e demais extensões como
      etapas próprias, conforme a leitura e a escrita de conteúdo usadas
      pelo core.
- [ ] Validar primeiro um jogo compatível com Classic Controller emulado,
      depois Wiimote emulado e Nunchuk. Expor apenas os perfis que o
      binário anunciar e que tenham sido testados. Ponteiro, gestos e
      Wiimote físico por Bluetooth passam por validações separadas.
      [Mapeamento e Wiimote no Dolphin libretro](https://docs.libretro.com/library/dolphin/#joypad)
- [ ] Testar gravação do save do Wii e isolamento do perfil gravável.
      Confirmar os caminhos usados na prática antes de avaliar
      sincronização com o Alvanista. Manter cartões do GameCube e dados
      do Wii locais nesta entrega.

### D — Renderizadores adicionais

- [ ] Avaliar Vulkan após o piloto OpenGL. Confirmar o contrato da revisão
      específica do Dolphin e testar se o caminho Vulkan do ReEmu o
      atende. Medir o custo de emulação e apresentação antes de oferecer a
      opção. [Vulkan no Dolphin](https://docs.libretro.com/library/dolphin/#vulkan) · [Plano Vulkan do ReEmu](12-vulkan-hw-render-fase2.md)
- [ ] Avaliar D3D11 no Windows depois da integração do Dolphin e da base
      Direct3D validada em outros cores. Testar GameCube e Wii com a DLL
      escolhida e oferecer o backend por escolha explícita após validar.
      [Roteiro Direct3D](24-direct3d-por-core.md)

## Critérios de liberação

- Um jogo representativo por plataforma anunciada abre pelo catálogo e
  pela biblioteca, mostra o primeiro quadro, aceita controle, pausa, salva,
  restaura, fecha e abre de novo. Validar save do jogo e save state em
  separado, incluindo persistência depois de reiniciar o app.
- A falta de `Sys`, arquivos de disco inválidos e falhas de contexto geram
  diagnóstico, e o processo filho pode ser encerrado mantendo o app
  disponível. A identificação não muda a plataforma de ISOs da biblioteca
  sem evidência. O smoke test automatizado usa conteúdo redistribuível
  quando houver uma fixture adequada. Os testes locais podem usar dumps
  do usuário. [Smoke test do catálogo](../../.github/workflows/catalog-smoke.yml)
- Registrar core, versão, renderizador e plataformas realmente testadas.
  Comparar vídeo e áudio em resolução nativa e com escala, medir tempo de
  quadro e aplicar o portão do projeto aos caminhos existentes. [Roteiro de desempenho](../perf/README.md)

Decisões de ABI, GPU, formatos e controles devem consultar a documentação
oficial e o código upstream da revisão usada. Mudanças estruturais em core
ou GPU seguem o aviso prévio do [plano unificado](19-plano-unificado.md) e a
[regra de fontes oficiais](../../CLAUDE.md).
