# Medir o desempenho dos jogos

Serve para provar que uma mudança não piorou os jogos. Os limites vêm de
[`19-plano-unificado.md`](../ai-context/19-plano-unificado.md), em "Portão de
desempenho". A referência fica em `baseline.json` e só vale para a máquina em que foi
medida.

## Antes de medir

- Pare o `tauri dev` e feche o que pesa no PC (navegador, gravação de tela).
- Use sempre a mesma tela, a mesma resolução e os mesmos jogos e configurações.
- Anote na seção "Referência atual" o PC, a resolução e os jogos usados.

A captura usa o perfil `dev`, porque só ele grava o log. Ele é mais lento que a versão
publicada, então serve para comparar antes e depois, não como desempenho final.

## Como medir

1. Abra o app pela captura:

   ```powershell
   .\scripts\perf-capture.ps1
   ```

2. Jogue cada cenário por **pelo menos 60 s**, em uma parte movimentada do jogo, sem
   abrir menus.
3. Feche o jogo e espere **pelo menos 10 s** antes de abrir o próximo. É essa pausa que
   separa os trechos. Se o jogo ficar em pausa por mais de 10 s, o trecho se divide.
4. Feche o app e gere o relatório, com os nomes na ordem em que jogou:

   ```powershell
   python scripts/perf-report.py target\perf\sessao.log --labels software,opengl,vulkan,shader --save docs/perf/baseline.json
   ```

## Cenários

| Nome | O que jogar | Shader |
|---|---|---|
| `software` | Core 2D, como o Snes9x | Nenhum |
| `opengl` | PPSSPP, que nesta máquina usa OpenGL, em cena 3D | Nenhum |
| `vulkan` | Flycast, que nesta máquina usa Vulkan, em cena 3D | Nenhum |
| `shader` | O mesmo jogo do `opengl` | O shader pesado que você usa |

O relatório mostra core, render e shader de cada trecho. **Confira se bate com o nome:**
o core escolhe o render pela configuração dele, e o shader pode estar atribuído ao jogo.

**Conferir à parte, sem medida automática:** abrir e fechar o menu de pausa e trocar de
jogo. Observe se pisca ou trava.

## Se a compilação falhar

O relatório recusa a sessão se o core-host não compilou, porque o jogo teria rodado com
um binário antigo. Se o link falhar mesmo sem compilação incremental, rode
`cargo clean -p core-host-desktop -p reemu-desktop` e repita.

## Depois de uma mudança

Repita com os mesmos jogos, gravando em outro arquivo, e compare:

```powershell
.\scripts\perf-capture.ps1 -Saida target\perf\depois.log
python scripts/perf-report.py target\perf\depois.log --labels software,opengl,vulkan,shader --compare docs/perf/baseline.json
```

O comando sai com erro se alguma medida passar do limite. Repita a medida antes de
concluir que piorou, porque um trecho isolado pode variar.

## O que é medido

| Medida | Significado | Limite |
|---|---|---|
| `int_med`, `int_p99` | Intervalo entre quadros: mediana da média e do pior 1% a cada segundo | 2% pior, ignorando até 0,2 ms |
| `run_med` | Tempo do `retro_run` | 2% pior, ignorando até 0,2 ms |
| `render_med` | Tempo para desenhar o quadro | 2% pior, ignorando até 0,2 ms |
| `perdidos_s` | Quadros perdidos por segundo | Até 0,05 a mais |
| `atrasados_s` | Quadros atrasados por segundo | Até 0,05 a mais |
| `audio_baixo` | Fração de segundos com menos de 95% das amostras de áudio esperadas | Até 0,05 a mais |

## Limitações

- **Áudio:** o log mostra as amostras que o core produziu, não falhas na saída de som.
  Estalos ainda precisam ser percebidos de ouvido.
- **Cores que rodam no processo do app:** não geram `perf core`, só as medidas de vídeo.
  O Flycast não é um deles: com Vulkan, ele roda no processo filho e gera as duas.
- **Alocações por quadro:** não são medidas aqui.

## Referência atual

Medida em 2026-10-30, em `baseline.json`, no perfil `dev`, numa RTX 3060 com tela de
3840×2160.

| Cenário | Jogo | Render | Shader |
|---|---|---|---|
| `software` | Core Supafaust | Software | Nenhum |
| `opengl` | PPSSPP (Tekken: Dark Resurrection) | OpenGL | Nenhum |
| `vulkan` | Flycast (Soulcalibur) | Vulkan | Nenhum |
| `shader` | PPSSPP (God of War: Ghost of Sparta) | OpenGL | `crt-guest-advanced` |

Repita com estes mesmos jogos. O jogo de cada cenário PPSSPP foi deduzido da ordem de
abertura no log; confira ao medir de novo.
