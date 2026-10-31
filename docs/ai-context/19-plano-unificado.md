# 19 — Plano unificado: robustez do projeto e menus para TV e controle

**Estado: planejado (2026-10-30). Nada foi implementado.** Junta as melhorias
estruturais com o plano de frontend do
[`18-experiencia-console-tv.md`](18-experiencia-console-tv.md), que continua
sendo o detalhe das etapas de interface.

## Regras

1. **O jogo não pode piorar.** Toda fase passa pelas medidas de "Portão de
   desempenho" antes de ser dada como pronta.
2. **Mudança estrutural em core, Vulkan ou OpenGL exige aviso ao dono do projeto
   antes de começar**, mesmo com a fase aprovada: o que muda, o risco e como será
   verificado. As mudanças conhecidas estão em "Mudanças que exigem aviso".
3. **GPU só com a documentação oficial**, citada no commit ou no histórico
   (`CLAUDE.md`).
4. **Uma mudança estrutural por vez, sem mudar comportamento junto.**
5. **Cada etapa fica só local.** Commit e push depois que o dono do projeto testar e
   aprovar aquela etapa; a aprovação de uma não vale para a próxima.

## Avaliação do estado atual

Base: estrutura, tamanhos e buscas no código em 2026-10-30, além da leitura de
trechos de `session.rs`, `video.rs` e `gpu/mod.rs`. Não é auditoria linha a linha.

### O que está bom e deve ficar

- **`crates/domain` sem I/O:** o `Cargo.toml` não importa wgpu, tauri, cpal e
  afins, como manda a arquitetura.
- **Core em processo filho descartável** (`core-host-desktop`, `core-ipc`):
  resolve cores não reentrantes e isola falhas.
- **Dependências sem ciclo aparente** entre os crates.
- **Caminho de vídeo já trabalhado:** sem alocação por quadro no caminho de
  software, ritmo adaptativo, `REEMU_PERF=1` para medir.
- **Máquina de estados do menu** já existe (`VideoMenu`).

### Pontos fracos

| Ponto | Medida | Impacto |
|---|---|---|
| Rota de core Vulkan no processo do app | Padrão no Linux e quando `REEMU_VK_CHILD=0`; no Windows o padrão é o processo filho desde o C3c, como o log do Flycast confirma | Nessa rota, crash do core derruba o app |
| CI não roda testes no Windows | Só `cargo check` cruzado, no Linux | A principal plataforma de uso só é testada na máquina local e na fumaça manual do catálogo |
| CI não valida GPU | 44 testes ignorados, 28 deles em `gpu/tests.rs` | Regressão em Vulkan ou OpenGL não é pega automaticamente |
| Sem linha de base de desempenho guardada | `REEMU_PERF` existe, sem valores registrados por versão nem benchmark | Não há como provar que uma mudança não piorou |
| `session.rs` | 2.549 linhas, 64 referências a travas e 85 usos | Difícil de analisar; falta confirmar quantas estão no caminho de quadro |
| `gpu/mod.rs` | 2.832 linhas, 67 funções | Mudança arriscada por ser tudo junto |
| `gl_context.rs` | 2.624 linhas, 77 `unsafe` | Mesmo problema, em FFI |
| Contrato Rust e TypeScript manual | 118 comandos, `tauri.ts` com 956 linhas | Erro só aparece com o app rodando |
| Erros como texto | 87 dos 118 comandos com `Result<_, String>` | Interface não sabe tratar nem traduzir |
| Lógica sem GPU dentro do app Tauri | `core_catalog.rs`, `scraping.rs` | Mais difícil de testar |
| Navegação só geométrica | `focusNav.ts` | Becos sem saída no controle (ver doc 18) |
| Estilos concentrados | `xbox.ts` com 1.868 linhas | Consistência depende de revisão manual |

Comandos e `Result<_, String>` foram contados por busca, então são aproximados.

## Mudanças que exigem aviso

Nenhuma delas começa sem o aviso ao dono do projeto.

| Mudança | Afeta | Risco | Decisão |
|---|---|---|---|
| Dividir `gpu/mod.rs` em módulos | Vulkan, OpenGL e shaders, pelo caminho de vídeo | Baixo se só mover código; dentro do mesmo crate não deve mudar o desempenho | Fase 8, só movendo código |
| Máquina de estados em `session.rs` | Todos os cores | Médio: pode mudar a ordem de carregar e descarregar, sensível em cores não reentrantes e Vulkan | Fase 8, só nas transições, fora do caminho de quadro |
| Reduzir travas em `session.rs` | Todos os cores, áudio | Alto: risco de deadlock e de corrida | Só com medição que aponte gargalo; hoje não há |
| Dividir `gl_context.rs` | Cores OpenGL | Médio: FFI e dependência de ordem de chamadas | Só se a Fase 8 anterior correr bem |
| Mudar a rota de core Vulkan | Cores Vulkan | Alto: é o caminho com anel de imagens e sincronização entre processos | **Fora do plano.** No Windows já roda no processo filho |
| Mover `gpu/` para crate próprio | Vulkan, OpenGL | Médio: pode afetar otimização entre crates | **Fora do plano**, sem ganho que compense |
| Trocar versão de wgpu ou ash | Vulkan, OpenGL | Alto | Fora do plano; tratar à parte, com a documentação da versão |
| Trocar a escala da interface (`uiScale.ts`) | **Vídeo nativo**: a janela do jogo pode depender dessas coordenadas | Médio: imagem deslocada ou fora de escala | Fase 3; conferir antes como a posição é calculada |
| Efeitos no menu de pausa | Passagem entre vídeo nativo e interface | Médio: piscada, queda de quadros ao abrir o menu | Efeitos pesados ficam fora do menu de pausa |

**Não afeta cores nem GPU:** tipos gerados, erros tipados, extração de catálogo e
scraping, telas e navegação fora do jogo. O comando binário `poll_frame` fica fora
da geração de tipos, para não mudar o caminho do `<canvas>`.

## Fases

### Fase 0 — Medir e proteger

Sem mudar o app.

- Registrar a linha de base com `REEMU_PERF=1`, no Windows, nos cenários de
  "Portão de desempenho".
- Job de testes em Windows real na CI, sem GPU.
- Fumaça do catálogo agendada, avisando se um core mudar de repositório ou de
  formato de versão.
- Roteiro local dos testes de GPU ignorados, para rodar antes de release.
- Referência do Xbox: feita a partir dos vídeos (etapa 0 do doc 18).

**Pronto quando:** houver números guardados por cenário e a CI rodar no Windows.

### Fase 1 — Contratos

- Tipos gerados de Rust para TypeScript com o `ts-rs` 12, só em `cargo test`, em
  `lib/bindings`. O `tauri-specta` para Tauri 2 ainda é versão candidata
  (`2.0.0-rc.26`), presa a uma versão exata do `specta`.
- O `ts-rs` não vê os comandos: `lib/commands.test.ts` confere os nomes. Os parâmetros
  continuam sem conferência.
- Erros com código (`app_error.rs`), usados primeiro em carregar jogo e em cores. Sem
  código, a interface segue as regras de texto de `lib/errors.ts`.
- **Ficam para depois:** `HotkeyBinding`, `ControllerEntry` e `ControllerMapping`, que
  usam tipos do crate `domain`; `ShaderFailure` e `HotkeyActionEvent`, que são eventos;
  e os códigos de erro nos comandos de shader.

**Pronto quando:** renomear um campo em Rust quebrar o `tsc -b`.

### Fase 2 — Controle sem becos sem saída

Etapas 1 e 2 do doc 18, com uma camada única de foco (zonas e memória por tela).

### Fase 3 — Densidade e tokens

Etapa 3 do doc 18, mais os tokens de design para tamanhos, foco e fundo escurecido.
O layout e os cards atuais ficam. **Exige aviso** por causa da `uiScale.ts`.

### Fase 4 — Configurações no modelo do console

Etapa 4 do doc 18.

### Fase 5 — Demais menus, biblioteca, jogo e entrada de dados

Etapas 5, 6 e 7 do doc 18, com lista virtualizada para bibliotecas grandes.

### Fase 6 — Extrair lógica sem GPU

`core_catalog`, `core_updates` e `scraping` em crates testáveis, um por vez,
aproveitando quando precisarem mudar. Não toca no carregamento de cores.

### Fase 7 — Acabamento e diagnóstico

Etapa 8 do doc 18, mais o diagnóstico exportável (log, versões, core, GPU e
configuração), gerado só sob pedido, e registro curto de decisões.

### Fase 8 — Estrutura de core e GPU

**Cada item exige aviso.** Vem por último porque precisa da Fase 0 e tem o menor
ganho para quem joga.

1. Dividir `gpu/mod.rs`, só movendo código.
2. Mapear travas de `session.rs` e separar o que está no caminho de quadro.
3. Máquina de estados do ciclo do jogo, só nas transições.

Se um item não trouxer ganho claro para manutenção, ele é cancelado.

## Portão de desempenho

**Cenários, no Windows:**

- Core 2D por software, sem shader.
- Core OpenGL (N64 ou Dreamcast).
- Core Vulkan.
- Jogo com shader pesado.
- Abrir e fechar o menu de pausa durante o jogo, e trocar de jogo.

**Medidas:** as linhas `perf core` e `perf vídeo` do `REEMU_PERF=1`, resumidas por
`scripts/perf-report.py`. Roteiro em [`docs/perf/README.md`](../perf/README.md). O
`perf core` já tem o percentil 99 e sai no terminal, não no log do app. Estalos de
áudio e alocações por quadro não são medidos automaticamente.

**Limites propostos, a ajustar depois da linha de base:**

- Tempo de quadro médio e percentil 99: no máximo 2% pior que a linha de base.
- Nenhuma falha de áudio nova e nenhuma alocação nova por quadro.
- Interface: 60 quadros por segundo navegando numa TV 4K e num PC modesto, com
  milhares de jogos.
- Abertura e memória: sem piora perceptível.

**Quando roda:** nas fases que tocam vídeo, jogo ou menu de pausa, e sempre antes de
release. As demais fases rodam só os cenários 1 e 5, como conferência.

## Riscos

| Risco | Prob. | Impacto | Como resolver sem afetar os jogos |
|---|---|---|---|
| Regressão de GPU passa despercebida (CI não cobre) | Alta | Alto | Roteiro local de GPU, portão e fumaça do catálogo antes de release |
| Mudança na `session.rs` causa deadlock ou crash ao trocar de jogo | Média | Alto | Só transições, testes de carregar e descarregar repetidos, um cenário de cada tipo de core |
| Crash de core derruba o app na rota dentro do processo | Existente, no Linux | Alto | Sem mudar a arquitetura: salvar dados antes de abrir o jogo, aviso no sino ao reabrir |
| Escala nova desalinha o vídeo nativo | Média | Alto | Conferir a origem das coordenadas, testar janela, tela cheia e monitores com DPI diferente |
| Efeitos da interface reduzem quadros no jogo | Média | Médio | Sem efeito pesado no menu de pausa, animar só `transform` e `opacity`, interface parada durante o jogo |
| Som da interface disputa o áudio do jogo | Baixa | Médio | Sons só fora do jogo no início; se entrarem no menu de pausa, testar áudio antes |
| Reescrever foco quebra a navegação atual | Média | Médio | Migrar tela por tela, com `audit-focus` e roteiro de controle |
| Tipos gerados atrapalham comandos especiais | Baixa | Baixo | Adoção gradual, `poll_frame` de fora |
| Perfil automático erra entre TV e PC | Média | Baixo | Opção manual, sem troca no meio do uso |
| Tabela de repositórios envelhece | Alta | Baixo | Verificação agendada; core fora da tabela só fica sem aviso |
| Trabalho grande demais para revisar | Média | Médio | Fases pequenas, um commit por mudança, reversível |

## Como reverter

- Uma mudança estrutural por commit, para `git revert` isolado.
- Mudanças de interface grandes atrás de preferência até a validação, quando viável.
- Antes de release, conferir os cenários do portão na versão candidata.

## Pendências de verificação

- Quantas travas de `session.rs` estão no caminho de quadro.
- De onde vêm as coordenadas do vídeo nativo em relação ao zoom.
