# 06 — Áudio (Dynamic Rate Control)

## Objetivo desta etapa

Implementar `AudioSink` (desktop, via `cpal`), com **Dynamic Rate
Control** pra sincronia áudio/vídeo — não resample fixo.

## Decisões relevantes

- **Dynamic Rate Control é obrigatório**, não opcional: ajuste a razão de
  resample em tempo real baseado no nível de preenchimento do buffer
  (buffer enchendo → resample um pouco mais rápido; esvaziando → um pouco
  mais devagar), dentro da margem `AudioConfig.rate_control_delta`
  (default 0.005 = ±0.5%, configurável pelo usuário).
- Identificação de dispositivo por **ID persistente do SO**, nunca por
  índice de enumeração (índice muda a cada hot-plug).
- **Fallback quando o dispositivo salvo não é encontrado**: usa o
  dispositivo padrão do sistema automaticamente E dispara um toast
  informativo (`ToastPublisher`, ver `07-frontend-react.md`) — nunca
  bloqueia a inicialização esperando o dispositivo salvo aparecer.
- Ao entrar em `MenuFocused`, o `AudioSink::pause()` deve **parar o
  stream**, não só silenciar/deixar de processar — evita zumbido de
  buffer.
- Linux: teste explicitamente contra PipeWire, PulseAudio (via camada de
  compatibilidade) e ALSA direto — `cpal` abstrai isso, mas o
  comportamento de latência varia entre os três.

## Estrutura sugerida

```
crates/core-loader-desktop/src/
  audio_sink.rs   -- implementa domain::audio::AudioSink usando cpal
  rate_control.rs  -- lógica de Dynamic Rate Control isolada e testável
                       sem precisar de um device de áudio real
```

Separe `rate_control.rs` do `audio_sink.rs` deliberadamente — a lógica de
ajuste de taxa é pura (função de "nível de buffer" → "fator de ajuste") e
deve ser testável sem hardware.

## Estado atual (2026-09-19 — `done`)

`crates/audio-desktop`:
- `rate_control.rs` — DRC puro (`RateControl::factor(fill)`, limitado a
  ±`rate_control_delta`).
- `sink.rs` — `CpalAudioSink`: ring buffer + resample **linear** de razão
  variável. `RateEstimator` estima a taxa REAL de chegada das amostras (o
  core nem sempre entrega a taxa que anuncia), limitado a ±12%. Dispositivo
  escolhido por `DeviceId` persistente; se o salvo não existir mais, cai no
  padrão do sistema.
- A factory do sink roda **na thread do core** (a `cpal::Stream` é `!Send`).

Integração: `emu-session` drena o áudio do core pro sink; pausa/foco chamam
`pause()`/`resume()`. `SET_SYSTEM_AV_INFO` em runtime atualiza a taxa (o N64
troca de taxa depois do boot — era a causa do áudio picotado, 2026-09-04).
`update_audio_config` aplica ao vivo via `session.reload_audio`, sem
reiniciar o jogo. Tela: `SettingsAudio`. Diagnóstico: `REEMU_AUDIO_DEBUG=1`
(fps, amostras por segundo, frames acima do orçamento).

Validado: N64 ~1 min sem underrun; sobram ~2 engasgos isolados por sessão
(30–55 ms), que o buffer de 250 ms quase absorve. **Falta**:
- sessão de 10+ min, pedida no critério de pronto, nunca medida formalmente;
- trocar pro `rubato` só se a qualidade do linear não bastar;
- nenhum toast quando o dispositivo salvo some — hoje a troca pro padrão é
  silenciosa (o critério de pronto pede toast).

## Depende de

`02-core-loader-desktop.md` (o core precisa estar produzindo samples de
áudio) e `03-tauri-desktop-shell.md` (pra pause/resume via foco).

## Critério de pronto

- Sessão de jogo longa (10+ minutos) sem cortes/glitches perceptíveis de
  áudio
- Trocar de dispositivo de áudio em runtime não corta o som abruptamente
- Desconectar o dispositivo salvo e reiniciar o app cai no padrão do
  sistema com um toast, sem travar
