# 24 — Direct3D por core no Windows

**Estado** — Backlog desde 2026-11-18. O ReEmu oferece software, OpenGL e
Vulkan aos cores. Este projeto adiciona gradualmente as interfaces libretro
D3D11 e D3D12 no Windows, com validação por core e por versão da DLL. O
[plano do PPSSPP](23-direct3d11-ppsspp.md) detalha o primeiro piloto.
Cada mudança estrutural segue o aviso prévio e o portão de desempenho
do [plano unificado](19-plano-unificado.md).

**Regra de decisão** — Consulte as fontes oficiais antes de decidir sobre
ABI, formatos, estados de recursos, sincronização, compartilhamento ou
compatibilidade. Use Microsoft Learn para o Direct3D e DXGI, os cabeçalhos
oficiais do libretro para o contrato do frontend e o upstream do core para
a versão correspondente à DLL. Registre as fontes e marque como não
verificado o que elas não cobrirem. [Instruções do projeto](../../CLAUDE.md) ·
[Índice de referências](REFERENCES.md#direct3d-11-e-12-no-windows).

## Ordem por core

| Ordem | Core | API no escopo | Validação específica |
| --- | --- | --- | --- |
| 1 | **PPSSPP** · PSP · `ppsspp_libretro` | **D3D11**, piloto | Confirmar a opção anunciada pela DLL e os assets do PSP. Comparar imagem e tempo de quadro a 1× e 2× com Vulkan e OpenGL. Conferir menu, troca de resolução, alpha da textura e save states. [Libretro Docs](https://docs.libretro.com/library/ppsspp/#hardware-rendering) · [Plano do piloto](23-direct3d11-ppsspp.md) |
| 2 | **SwanStation** · PS1 · `swanstation_libretro` | **D3D11**, segundo core. **D3D12**, piloto da segunda API | Verificar as duas opções na DLL do catálogo. Conferir FMV, transparências, resolução interna e troca de disco. Validar D3D12 em separado, depois de estabilizar a base compartilhada D3D11. [Build Windows do SwanStation](https://github.com/libretro/swanstation/blob/main/Makefile.libretro) |
| 3 | **Flycast** · Dreamcast, NAOMI e Atomiswave · `flycast_libretro` | **D3D11** | Conferir a negociação de preferência na DLL. Comparar Dreamcast e um título arcade com Vulkan. Testar framebuffer completo e ordenação de transparências, além de persistência de VMU no Dreamcast. [Código libretro do Flycast](https://github.com/flyinghead/flycast/blob/master/shell/libretro/libretro.cpp) · [Relato upstream sobre framebuffer](https://github.com/flyinghead/flycast/issues/2284) |
| 4 | **LRPS2** · PS2 · `pcsx2_libretro` | **D3D11 e D3D12**, aceitos separadamente | Usar BIOS e cartões locais. Conferir cenas entrelaçadas, fim de sessão e carga de GPU em resolução nativa e ampliada. Investigar o caso documentado de tela preta no D3D11. Para D3D12, partir da base validada no SwanStation. Comparar com Vulkan. [Libretro Docs](https://docs.libretro.com/library/lrps2/) |
| 5 | **Dolphin** · GameCube e Wii · `dolphin_libretro` | **D3D11**, avaliação posterior | O catálogo atual não inclui o core. A avaliação começa pelo suporte local a GameCube e Wii, formatos, assets `Sys`, entrada e smoke test. Depois, conferir dois jogos representativos, um de cada plataforma, antes de expor D3D11. [Plano do Dolphin](25-dolphin-libretro.md) · [Libretro Docs](https://docs.libretro.com/library/dolphin/#d3d11) |

O recorte considera o port **libretro** e o binário distribuído. Cores que
entregam pixels por software continuam usando o caminho atual, assim como
Mupen64Plus-Next, ParaLLEl N64, Beetle PSX HW, Kronos, YabaSanshiro e
melonDS DS nas rotas já disponíveis. Um novo candidato entra após
confirmar em fonte oficial que seu port implementa a interface e testar a
negociação na DLL exata. [Catálogo do ReEmu](../../crates/core-catalog/src/catalog.rs)

## Fases de implementação

### A — Inventário do binário e referência

- [ ] Por combinação core/API, registrar origem da DLL, versão, opções
      anunciadas, contexto libretro solicitado e versão da interface
      negociada. Registrar também a GPU e o driver do teste.
- [ ] Escolher a ROM e a cena de teste, guardar a referência em Vulkan ou
      OpenGL que já funciona para o core e usar a mesma configuração de jogo,
      resolução e shader na comparação.

### B — D3D11 no PPSSPP e mais um core

- [ ] No `core-loader-desktop`, oferecer `SET_HW_RENDER` e
      `GET_HW_RENDER_INTERFACE` com o contrato D3D11 somente quando o
      backend estiver disponível e a sessão o tiver selecionado.
      Criar e liberar device, contexto e recursos na thread do core.
      Usar a convenção de entrega da v1 para o primeiro piloto.
      [Cabeçalho libretro D3D11](https://github.com/libretro/RetroArch/blob/master/libretro-common/include/libretro_d3d11.h) · [Plano PPSSPP](23-direct3d11-ppsspp.md)
- [ ] Executar D3D11 no `reemu-core-host`. Reutilizar o transporte de
      quadros por memória compartilhada e o compositor wgpu, com captura
      via staging e conversão de formato. Separar o código comum das
      opções e do ciclo de vida de cada core.
- [ ] Depois de validar PPSSPP, repetir o fluxo com SwanStation. Usar o
      Flycast e o LRPS2 para validar rotas adicionais, cada uma com sua
      matriz de compatibilidade. Medir antes de escolher se vale a pena
      implementar a interface v2 ou exportar texturas sem leitura pela CPU.

### C — D3D12 no SwanStation e no LRPS2

- [ ] Criar a integração D3D12 no processo filho, com device, fila, command
      lists, estados de recursos, fences e ciclo de vida das texturas.
      Negociar a versão que a DLL aceita. Na v1, submeter a cópia na
      thread do core antes de retornar do callback. Na v2, respeitar a
      fence e o índice de sincronização acordado antes de ler ou liberar
      uma textura. [Cabeçalho libretro D3D12](https://github.com/libretro/RetroArch/blob/master/libretro-common/include/libretro_d3d12.h) · [Filas e command lists](https://learn.microsoft.com/en-us/windows/win32/direct3d12/command-queues-and-command-lists)
- [ ] Começar com buffer de readback. Calcular o layout com
      `GetCopyableFootprints`, aguardar a fence que sinaliza a conclusão
      da cópia e então publicar os pixels pelo transporte atual. Usar
      a camada de depuração e DRED para investigar perda de dispositivo.
      [Barreiras](https://learn.microsoft.com/en-us/windows/win32/direct3d12/using-resource-barriers-to-synchronize-resource-states-in-direct3d-12) · [Readback](https://learn.microsoft.com/en-us/windows/win32/direct3d12/readback-data-using-heaps) · [Layout](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/nf-d3d12-id3d12device-getcopyablefootprints) · [DRED](https://learn.microsoft.com/en-us/windows/win32/direct3d12/use-dred)
- [ ] Usar o SwanStation como primeiro core. Reaproveitar a infraestrutura
      no LRPS2 após validar ciclo de vida, imagem e sincronização. Se o
      readback limitar a entrega, medir compartilhamento e sincronização
      com a API efetiva do compositor. [Interop D3D12](https://learn.microsoft.com/en-us/windows/win32/direct3d12/direct3d-12-with-direct3d-11--direct-2d-and-gdi)

### D — Seleção e diagnóstico

- [ ] Distinguir **capacidade do core**, **opção selecionada** e **backend
      efetivo da sessão**. O valor persistido hoje descreve a última rota
      que o core usou. Ao acrescentar D3D11/D3D12, revisar a enumeração
      `RenderBackend`, o IPC, a conversão no banco e os tipos da UI,
      preservando as instalações e configurações existentes.
- [ ] Expor Direct3D no Windows por core, sob escolha explícita, quando a
      combinação de DLL e backend tiver sido validada. Aplicar a troca ao
      reabrir o jogo. Registrar o backend efetivo no diagnóstico.
      Na falha de inicialização, informar a causa e oferecer retorno ao
      backend anterior em nova sessão. Manter o padrão atual de cada core
      enquanto os testes não justificarem uma mudança.

## Critérios de liberação

- **Imagem e ciclo de sessão.** Verificar primeiro quadro, resolução e
  proporção, canal alpha, quadros repetidos, pausa, troca de jogo, save state,
  save do jogo e encerramento. Conferir shaders e molduras com a rota real
  de vídeo. Se a DLL tiver restrição de save state entre renderizadores,
  registrar a limitação e preservar os arquivos.
- **Estabilidade.** Testar com camada de depuração, validar encerramento e
  perda de dispositivo, e confirmar que o processo filho pode falhar
  mantendo o aplicativo aberto. Registrar separadamente cada combinação
  aprovada de core e API.
- **Desempenho.** Usar o [roteiro do projeto](../perf/README.md) para
  conferir regressões nas rotas existentes. Para cada rota nova, medir
  tempo de quadro p95/p99, quadros perdidos, áudio, uso de CPU/GPU e custo
  do readback. Repetir no hardware disponível, com casos de NVIDIA, AMD e
  Intel incluindo GPU integrada. Se a rota nova não for competitiva com
  o backend já disponível, manter o modo experimental até entender a
  causa ou documentar o caso de compatibilidade em que vale utilizá-lo.
