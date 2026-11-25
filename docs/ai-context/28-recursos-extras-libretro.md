# 28 — Multiplayer e recursos extras dos cores Libretro

**Status:** backlog desde 2026-11-24.

O plano cobre partidas com controles sincronizados, redes dos consoles
emulados, cabo Link, conquistas, troca de mídia, rewind, redução de latência
e periféricos. As fases de conquistas e rewind usam os planos 16 e 17.

## Base do ReEmu

O ReEmu já carrega opções de core, reconhece `.m3u` no scanner e oferece save
states e cheats por código. A execução normalmente passa pelo
`core-host-desktop`, com mensagens em `core-ipc`. `emu-session` também tem o
caminho Vulkan no processo do app.
O loader reconhece `SET_SUBSYSTEM_INFO`, mas ainda precisa guardar os descritores
e resolver `retro_load_game_special` para iniciar jogos com mais de uma mídia.
[Catálogo](../../crates/core-catalog/src/catalog.rs) ·
[Callbacks](../../crates/core-loader-desktop/src/ffi_state.rs) ·
[Formatos da biblioteca](../../crates/library-scan/src/systems.rs).

## 0. Detectar recursos por core e versão

- [ ] Inventariar as capacidades anunciadas pelo binário e as condições por
  sistema, jogo, versão, BIOS, opções e renderizador. Registrar separadamente
  o suporte documentado, o detectado em execução e o validado no ReEmu.
- [ ] Resolver callbacks opcionais no loader. Guardar ponteiros e operar o core
  na thread de emulação. Passar pelo IPC apenas comandos, descritores copiados,
  resultados e eventos que o app usa. Limpar os dados ao fechar o conteúdo, ao
  trocar de core e após falha do host.
- [ ] Expor ações contextuais nos detalhes e no menu de pausa com componentes e
  ícones Fluent UI 2, acesso por teclado e controle e traduções pt-BR, en e es.
  Informar quando falta mídia, configuração, validação ou suporte nessa versão.
- [ ] Exercitar detecção, chamadas e limpeza com o core de teste, depois
  confirmar cada recurso em um binário real. Registrar a versão ou hash do
  binário e as opções necessárias no [inventário Libretro](inventario-libretro.md).

[Header Libretro](https://raw.githubusercontent.com/libretro/libretro-common/master/include/libretro.h) ·
[Opções por cenário no ReEmu](21-opcoes-cores-por-cenario.md).

## 1. Redes próprias do PSP e do DS

- [ ] **PPSSPP.** Conferir as opções WLAN e PRO Ad Hoc no build instalado.
  Usar a configuração de core existente para testar dois clientes, primeiro
  em LAN, depois por um servidor compatível com o build. Validar que ambos
  entram pelo menu do próprio jogo, completam uma partida e voltam a jogar
  após sair. Testar perfil de rede desligado e falha de conexão.
- [ ] **PPSSPP.** Depois da validação, oferecer ativação da rede e configuração
  de servidor com escopo por core ou jogo. Confirmar requisitos de reinício e
  serviços compatíveis para cada versão antes de os apresentar na interface.
- [ ] **melonDS DS.** Validar a Nintendo Wi-Fi Connection em um jogo compatível
  com servidor comunitário indicado pelo core. Conferir firmware, DNS e
  identidade de rede exigidos pelo título. Exibir os requisitos e a opção
  de servidor confirmada no build.
- [ ] **melonDS DS.** Validar o modo de multiplayer local entre dois DS em LAN
  cabeada, depois da fase 4 se o build usar Netpacket. Testar os MACs, a entrada
  no jogo e a desconexão. Registrar o requisito de latência próprio desse modo.

Fontes oficiais · [PPSSPP Libretro](https://docs.libretro.com/library/ppsspp/#network) ·
[Rede no PPSSPP](https://www.ppsspp.org/docs/multiplayer/quickstart/) ·
[Wi-Fi e LAN do melonDS DS](https://docs.libretro.com/library/melonds_ds/#wi-fi).

## 2. Cabo Link local e múltiplas mídias

- [ ] **TGB Dual e DoubleCherryGB.** Validar duas unidades emuladas usando as
  opções do core. Mapear os dois controles, conferir o layout das telas e provar
  uma interação Link em jogo compatível. Verificar a persistência da SRAM após
  sair e reabrir a sessão com o mesmo jogo em ambos os lados.
- [ ] **SameBoy.** Testar primeiro o modo de um cartucho com dois jogadores.
  Guardar os descritores do subsistema e chamar `retro_load_game_special` para
  dois cartuchos. Oferecer seleção a partir da biblioteca ou de arquivo, com
  validação de cada slot. Testar dois saves independentes, inclusive ao inverter
  os cartuchos e ao cancelar ou falhar na abertura do segundo.
- [ ] **melonDS DS.** Reusar a seleção de subsistemas para o cartucho GBA no
  Slot-2 e seu save opcional. Validar um título DS que reaja ao cartucho inserido,
  o fluxo com slot vazio e a proteção do save já existente do jogo de GBA.

Fontes oficiais · [TGB Dual](https://docs.libretro.com/library/tgb_dual/) ·
[DoubleCherryGB](https://github.com/TimOelrichs/doublecherryGB-libretro) ·
[SameBoy](https://docs.libretro.com/library/sameboy/#link) ·
[Subsistemas do melonDS DS](https://docs.libretro.com/library/melonds_ds/#subsystems).

## 3. Netplay por controles sincronizados

- [ ] **Piloto com Snes9x.** Repetir a mesma sequência de entradas em dois
  processos, confirmar determinismo e medir captura, restauração e reexecução
  antes de implementar o rollback. Usar a base de serialização e respeitar os
  quirks do core. Manter o histórico limitado em RAM no processo de emulação.
- [ ] **Sessão de dois jogadores em LAN.** Definir host, posições dos jogadores,
  quadro inicial e negociação de versão, ROM, BIOS e opções que alterem a
  emulação. Enviar entradas por quadro e corrigir entradas atrasadas com
  restauração e reexecução, mantendo áudio, vídeo e saves consistentes.
  Definir as regras para pausa, reset, cheats, save states e saída de sessão.
- [ ] **Estabilidade e interface.** Criar os fluxos Hospedar e Entrar, com estado
  da conexão, incompatibilidade, perda de sincronismo e desconexão. Testar
  atraso, variação de latência, perda de conexão e retorno ao menu. Comparar
  CPU, RAM, tempo por quadro e falhas de áudio com a mesma sessão local.
- [ ] **Outros cores.** Validar FCEUmm, Genesis Plus GX e FinalBurn Neo por
  build, jogo e modo multiplayer. No FBNeo, conferir versão do romset e BIOS.
- [ ] **Internet.** Após o piloto em LAN, testar conexão direta. Avaliar convite,
  descoberta, relay e espectadores em etapas próprias, conforme a necessidade
  medida. Registrar a decisão de interoperar ou ter protocolo próprio antes
  de prometer conexão com outros frontends. Usar a documentação e uma
  implementação original, respeitando as licenças do ReEmu e das dependências.

Fontes oficiais · [Protocolo e rollback](https://docs.libretro.com/development/retroarch/netplay/) ·
[Snes9x](https://docs.libretro.com/library/snes9x/#features) ·
[FCEUmm](https://docs.libretro.com/library/fceumm/#features) ·
[Genesis Plus GX](https://docs.libretro.com/library/genesis_plus_gx/#features) ·
[FinalBurn Neo](https://docs.libretro.com/library/fbneo/#features).

## 4. Redes emuladas via Netpacket

- [ ] Implementar `RETRO_ENVIRONMENT_SET_NETPACKET_INTERFACE` conforme o
  header da versão alvo. Reusar os fluxos de sala e o transporte disponível,
  negociando a versão de protocolo do core e respeitando o ciclo de conexão,
  recebimento, envio e fechamento dos callbacks. Validar tamanhos e frequência
  dos pacotes. Aplicar as restrições de manipulação do tempo da API na sessão.
- [ ] **DoubleCherryGB.** Emular um Game Boy por participante e testar a troca
  entre cartuchos compatíveis, com persistência dos dois saves e tratamento de
  desconexão. Para partidas com várias unidades emuladas no mesmo core, validar
  o modo de sincronização por estados da fase 3.
- [ ] **gpSP.** Validar o Wireless Adapter de GBA em FireRed, LeafGreen e Emerald.
  Registrar combinações de jogos e versões testadas e conferir troca, saída e
  persistência dos saves por participante.
- [ ] **DOSBox Pure.** Validar IPX em um jogo compatível e documentar por build
  os modos Ethernet e modem que passarem nos testes. Confirmar entrada, troca
  de dados durante a partida e encerramento com duas instâncias.
- [ ] **melonDS DS.** Confirmar o contrato de rede do build alvo e executar o
  cenário de LAN cabeada da fase 1 quando o transporte estiver disponível.

Fontes oficiais · [Header Libretro](https://raw.githubusercontent.com/libretro/libretro-common/master/include/libretro.h) ·
[gpSP e DOSBox Pure](https://www.libretro.com/index.php/retroarch-1-17-new-netplay-features/) ·
[Trocas via rede no DoubleCherryGB](https://github.com/TimOelrichs/doublecherryGB-libretro#link-cable-trading-over-network).

## 5. Conquistas, placares e presença

- [ ] Executar as fases do [plano RetroAchievements](16-retroachievements.md).
  Começar com `SET_MEMORY_MAPS`, identificação do conteúdo e `rc_client` no
  processo que executa o core. Usar o HTTPS do app e o cofre do sistema para o
  token. Validar login, identificação e desbloqueio softcore com Snes9x, depois
  expandir para conteúdo compatível em FinalBurn Neo e PCSX ReARMed.
- [ ] Exibir conquistas e placares, incluindo tentativa ativa, envio, falha e
  ranking. Manter rich presence com a sessão do `rc_client`. Validar persistência
  do progresso nos save states, queda da rede, retomada e encerramento da conta.
- [ ] Validar hardcore com a equipe RetroAchievements e aplicar as restrições
  no backend. Verificar as regras para rewind, cheats, carregamento de estado,
  netplay e execução especulativa antes de habilitar combinações de recursos.

Fontes oficiais · [Integração do rc_client](https://github.com/RetroAchievements/rcheevos/wiki/rc_client-integration) ·
[Identificação de jogos](https://docs.retroachievements.org/developer-docs/game-identification.html).

## 6. Troca de disco no menu de pausa

- [ ] Registrar as interfaces Libretro básica e estendida de Disk Control no
  loader. Enviar pelo IPC o estado da bandeja, índice, contagem e rótulos
  disponíveis. Implementar abertura, seleção e fechamento na thread do core,
  propagando falhas e atualizando o estado da interface após cada operação.
- [ ] Validar com playlist `.m3u` no PCSX ReARMed, depois no Beetle PSX HW. Usar
  mídia de teste com caminhos relativos, nomes com espaços e entradas ausentes.
  Confirmar troca em jogo, saída e reabertura, cartão de memória compartilhado
  entre discos e associação do save state ao disco correto.
- [ ] Estender a outros cores quando o callback e o cenário real forem validados.
  Usar a mesma identidade multidisco para biblioteca, saves e conquistas.

Fontes oficiais · [Header Libretro](https://raw.githubusercontent.com/libretro/libretro-common/master/include/libretro.h) ·
[PCSX ReARMed](https://docs.libretro.com/library/pcsx_rearmed/) ·
[Beetle PSX HW](https://docs.libretro.com/library/beetle_psx_hw/).

## 7. Rewind, run-ahead e quadros preemptivos

- [ ] Implementar o [plano de rewind](17-cheats-rewind.md#fase-3--rewind-backlog)
  com histórico opcional limitado em RAM. Validar primeiro em NES, SNES, Mega
  Drive e GBA. Ao soltar o atalho, confirmar a retomada de vídeo, áudio,
  controle e persistência. Medir o custo antes de expandir para outros cores.
- [ ] Prototipar run-ahead de um quadro no FinalBurn Neo. Medir a latência até a
  resposta visível, tempo de emulação e falhas de áudio. Verificar restauração
  repetível e isolar os efeitos de quadros especulativos sobre áudio, SRAM,
  conquistas e periféricos. Liberar por jogo após comprovar o ganho.
- [ ] Testar quadros preemptivos no mesmo core, com reexecução quando a entrada
  mudar. Medir o custo sob entradas frequentes e comparar com run-ahead.
  Expor os modos opcionais por core e versão quando forem estáveis. Definir
  compatibilidade com rewind, netplay e hardcore antes de habilitar cada modo.

Fontes oficiais · [Run Ahead](https://docs.libretro.com/guides/runahead/) ·
[Quadros preemptivos](https://www.libretro.com/index.php/retroarch-1-15-0-release/) ·
[Recursos do FinalBurn Neo](https://docs.libretro.com/library/fbneo/#features).

## 8. Periféricos

- [ ] **Flycast.** Mapear o dispositivo Light Gun com coordenadas da área de
  jogo, gatilho e disparo fora da tela. Testar com mouse ou dispositivo
  compatível, dois jogadores quando o jogo aceitar e calibração após mudar
  resolução, proporção, moldura ou modo de tela cheia.
- [ ] **DoubleCherryGB.** Validar o Game Boy Printer em um jogo que imprima.
  Identificar onde o build grava o PNG, listar as impressões e permitir abrir
  ou exportar pelo ReEmu, preservando arquivos anteriores após novas impressões.
  Verificar comportamento após restaurar estados e sair da sessão.
- [ ] Reusar a seleção de dispositivos e os metadados do loader para expor
  periféricos por core e por jogo. O Slot-2 do melonDS DS entra na fase 2.

Fontes oficiais · [Flycast](https://docs.libretro.com/library/flycast/) ·
[Versões do DoubleCherryGB](https://github.com/TimOelrichs/doublecherryGB-libretro/releases).

## Ordem de entrega e validação

1. Começar pela detecção de capacidades e pelo piloto ad hoc do PPSSPP. A troca
   de disco e o Link local podem avançar assim que os callbacks respectivos
   estiverem disponíveis. Conquistas e rewind seguem seus planos próprios.
2. Entregar o piloto Snes9x em LAN antes de abrir salas na internet. Usar a
   infraestrutura de sessões para Netpacket e ampliar os cores por resultado.
3. Liberar configurações de latência e periféricos core a core, após medir no
   aplicativo e fechar os casos de persistência e encerramento.

Cada entrega deve cobrir uso pela interface, falha ou cancelamento, segunda
execução e regressão no modo local. Registrar sistema, build, conteúdo, opções,
renderizador quando aplicável e resultado. Para recursos que afetam o loop de
emulação, guardar também a linha de base e os resultados de CPU, RAM, tempo por
quadro e falhas de áudio. A validação inicial é no Windows. Todos os testes em
Linux x64 ficam na [tarefa central de validação Linux](../../TASKS.md#testes-em-linux-x64).
