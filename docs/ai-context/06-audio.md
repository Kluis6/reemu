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

## Estado atual (2026-08-27 — `in-progress`)

`crates/audio-desktop`:
- `rate_control.rs` — DRC **puro** (`RateControl::factor(fill) -> f32`,
  limitado a ±`rate_control_delta`). 6 testes.
- `sink.rs` — `CpalAudioSink` (`domain::audio::AudioSink`): cpal 0.18, ring
  buffer, resample **linear de razão variável** com estado entre chamadas,
  fallback pro device padrão (match por `DeviceId` persistente). `pause()`
  para a stream de fato (`stream.pause()`).
- `domain::audio::AudioSink` não é mais `Send + Sync` (a `cpal::Stream` é
  `!Send`); `emu-session::SessionConfig.audio_sink` é uma factory `Send` que
  constrói o sink **na thread do core**.
- Wiring: `emu-session` drena `core.drain_audio()` → `sink.push_samples(...,
  core_sample_rate)`; `FocusController`/pause → `sink.pause()`/`resume()`.
- App: lê `AudioConfig` do SQLite no startup e passa pra factory. Stream cpal
  abre OK neste sistema (verificado).

Falta: verificar sessão longa sem glitch (core real + ouvir); trocar `rubato`
se a qualidade do linear não bastar; comando "aplicar config de áudio ao vivo".

## Depende de

`02-core-loader-desktop.md` (o core precisa estar produzindo samples de
áudio) e `03-tauri-desktop-shell.md` (pra pause/resume via foco).

## Critério de pronto

- Sessão de jogo longa (10+ minutos) sem cortes/glitches perceptíveis de
  áudio
- Trocar de dispositivo de áudio em runtime não corta o som abruptamente
- Desconectar o dispositivo salvo e reiniciar o app cai no padrão do
  sistema com um toast, sem travar
