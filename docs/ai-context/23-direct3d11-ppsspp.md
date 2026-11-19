# 23 — Direct3D 11 para o PPSSPP

**Estado** — Backlog desde 2026-11-18. O primeiro alvo é o core
`ppsspp_libretro` no Windows. O plano envolve o loader de cores e o caminho
de vídeo, por isso a implementação segue o aviso prévio do plano unificado.

O [roteiro por core](24-direct3d-por-core.md) organiza a expansão do
piloto para outras DLLs e para Direct3D 12.

## Suporte atual

O ReEmu oferece aos cores renderização por software, OpenGL e Vulkan.
Seu compositor usa wgpu e recebe o quadro pelo caminho de vídeo do app.
Dar a um core acesso ao Direct3D envolve um contrato próprio do libretro,
mesmo quando a GPU e o driver do Windows já oferecem Direct3D.

O PPSSPP anuncia D3D11 no Windows. No ReEmu já existe uma rota Vulkan
validada para ele, selecionada com `ppsspp_backend=vulkan`. Ela oferece uma
referência para comparar imagem, desempenho e latência.

| Core libretro | Direct3D documentado no Windows |
| --- | --- |
| PPSSPP (PSP) | D3D11. [Libretro Docs](https://docs.libretro.com/library/ppsspp/#hardware-rendering) |
| Flycast (Dreamcast, NAOMI e Atomiswave) | D3D11 no port libretro. Conferir a DLL do catálogo. [Código oficial](https://github.com/flyinghead/flycast/blob/master/shell/libretro/libretro.cpp) |
| Dolphin (GameCube e Wii) | D3D11. [Libretro Docs](https://docs.libretro.com/library/dolphin/#d3d11) |
| LRPS2 (PS2) | D3D11 e D3D12. A documentação registra tela preta em algumas GPUs na rota D3D11. [Libretro Docs](https://docs.libretro.com/library/lrps2/) |
| SwanStation (PS1) | D3D11 e D3D12 no código de build para Windows. Conferir a DLL distribuída quando for avaliar a integração. [Makefile do core](https://github.com/libretro/swanstation/blob/main/Makefile.libretro) |

## Referências oficiais e contrato

- Criar o dispositivo e o contexto imediato com `D3D11CreateDevice`.
  O processo filho precisa renderizar fora da tela. A apresentação continua
  na janela de vídeo do ReEmu. [Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-d3d11createdevice)
- O libretro distingue `RETRO_HW_CONTEXT_D3D11` do identificador da interface
  obtida com `GET_HW_RENDER_INTERFACE`. O cabeçalho D3D11 define o device,
  o contexto, o nível de recurso e o ponteiro de `D3DCompile`. [ABI libretro](https://github.com/libretro/libretro-common/blob/master/include/libretro.h) · [Cabeçalho D3D11](https://github.com/libretro/RetroArch/blob/master/libretro-common/include/libretro_d3d11.h)
- Na interface v1, o PPSSPP deixa a imagem vinculada ao slot 0 de recursos
  do pixel shader durante o callback de vídeo. A captura precisa acontecer
  nesse callback, na thread que executa o core, antes de o core desvincular
  a imagem. O contexto imediato exige coordenação de acesso entre threads.
  [Código do PPSSPP](https://github.com/hrydgard/ppsspp/blob/c1128ce35a464f58361bea4f457b65d3e5a1b25b/libretro/LibretroD3D11Context.cpp) · [Contextos imediato e diferido](https://learn.microsoft.com/en-us/windows/win32/direct3d11/overviews-direct3d-11-render-multi-thread-render)
- A interface v2 é negociada explicitamente. Ela recebe a textura por
  `set_texture` e define regras de acesso ao contexto e de reutilização das
  texturas por índice de sincronização. O código recente do PPSSPP aceita
  v1 e v2. A DLL escolhida para a validação determina quais caminhos podem
  ser exercitados. [Negociação e sincronização no cabeçalho](https://github.com/libretro/RetroArch/blob/master/libretro-common/include/libretro_d3d11.h) · [PPSSPP na revisão conferida](https://raw.githubusercontent.com/hrydgard/ppsspp/c1128ce35a464f58361bea4f457b65d3e5a1b25b/libretro/LibretroD3D11Context.cpp)
- A rota de leitura da GPU pode copiar para texturas de staging e ler as
  linhas pelo `RowPitch` retornado por `Map`. Medir a espera pela GPU e tratar
  formatos e amostras antes de copiar. [Staging](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ne-d3d11-d3d11_usage) · [CopyResource](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-copyresource) · [Map](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-map)

## Fases previstas

1. **Confirmar a DLL e o contrato.** Registrar a versão e a origem da DLL
   Windows do catálogo, consultar as opções que ela anuncia e verificar a
   chave e o valor efetivos para D3D11. Medir Vulkan e OpenGL na mesma ROM,
   resolução e GPU para estabelecer a referência.
2. **Negociar o backend no processo filho.** Implementar os tipos C do
   libretro no loader Windows e o ciclo de vida do device, do contexto e das
   referências COM. Começar pela v1 na thread do core. Ativar D3D11 apenas
   com escolha explícita. Reportar claramente a falha de inicialização e
   permitir que o usuário volte a Vulkan ou OpenGL.
3. **Exibir o primeiro quadro pela rota já existente.** Ler a saída D3D11
   para a CPU e entregar RGBA pelo anel de quadros atual. Reutilizar as
   texturas de staging, tratar `RowPitch`, canais, alpha e mudanças de
   resolução. Medir a espera em `Map` e a latência de qualquer fila antes de
   anunciar comparações de desempenho.
4. **Validar no aplicativo.** Testar abrir, pausar, redimensionar, alternar
   jogos, salvar e restaurar estado e encerrar o core. Conferir imagem,
   áudio e shaders do app. Ativar a camada de depuração D3D11 nos testes e
   comparar tempos de quadro p95/p99, latência, CPU e GPU com Vulkan e
   OpenGL, na resolução nativa e com escala. Começar pela NVIDIA disponível,
   depois verificar AMD e Intel, incluindo GPU integrada. Conferir regressões
   em Vulkan, OpenGL e software. [Camada de depuração](https://learn.microsoft.com/en-us/windows/win32/direct3d11/using-the-debug-layer-to-test-apps)
5. **Avaliar otimização se a cópia for um gargalo.** Verificar negociação
   v2, compatibilidade do adaptador e os mecanismos de importação e
   sincronização entre D3D11 e o compositor. Compartilhar uma textura
   entre dispositivos D3D11 é um caso diferente de importá-la num
   compositor de outra API. Manter a rota de leitura como alternativa
   até medir a integração. [Compartilhamento em D3D11](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag) · [OpenSharedResource1](https://learn.microsoft.com/en-us/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11device1-opensharedresource1)

**Critério da primeira entrega** — PPSSPP seleciona D3D11 explicitamente,
renderiza e troca de sessão sem erros da camada de depuração e tem resultados
comparáveis ao teste de referência. D3D12 e a habilitação de outros cores
exigem validação própria.
