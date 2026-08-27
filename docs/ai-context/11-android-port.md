# 11 — Port Android

## Objetivo desta etapa

Portar o app funcional (desktop) pro Android, criando `core-loader-mobile`
e adaptando as divergências já identificadas — só comece esta etapa
depois do desktop estar funcional ponta a ponta.

## Decisões relevantes (importante: leia antes de assumir restrições)

- **App não é distribuído na Google Play** — isso remove duas restrições
  que normalmente existiriam: (1) a política da Play Store contra
  download dinâmico de código, e (2) o incentivo a manter
  `targetSdkVersion` alto.
- **`targetSdkVersion` deve ser configurado baixo (ex: 28)** no
  `AndroidManifest.xml`, especificamente pra evitar a restrição de
  `dlopen()` de bibliotecas nativas baixadas em runtime que o Android
  10+ (API 29+) impõe a apps com target ≥29. Não configure um
  `targetSdkVersion` moderno "por segurança" — isso quebraria o requisito
  de download dinâmico de cores que já foi decidido.
- Existe um piso mínimo: o Android 14+ pode bloquear instalação de apps
  com `targetSdkVersion` abaixo de um valor do fabricante (tipicamente
  23) — não vá abaixo disso.
- **Distribuição de cores**: mesmo fluxo do desktop (`10-core-catalog.md`,
  buildbot em tempo real) — não implemente um catálogo restrito/bundlado
  aqui, essa alternativa foi descartada.
- **Isso é dívida técnica documentada**: se uma versão futura do Android
  apertar ainda mais o piso mínimo de `targetSdkVersion` aceito, essa
  decisão precisará ser revisitada. Não é permanente por garantia.

## Divergências reais (implementar aqui)

| Camada | Desktop | Aqui (Android) |
|---|---|---|
| Surface de vídeo | Child window nativa | `SurfaceView`/`TextureView` via JNI |
| Áudio | `cpal` | Oboe |
| Input | `gilrs` + SDL_GameControllerDB | API `InputDevice` nativa; cobertura MVP = só Xbox/PlayStation via Bluetooth (Abordagem A) — resto cai no fluxo manual de binding já existente (reaproveitar componente de `05-input-hotkeys.md`) |
| Carregamento de core | `libloading` | `dlopen` (mesmo mecanismo, viável pelo `targetSdkVersion` baixo) |

## Estrutura sugerida

```
crates/core-loader-mobile/
  src/
    lib.rs
    jni_bridge.rs       -- pontes JNI pra SurfaceView, InputDevice, Oboe
    core_loader_impl.rs   -- reaproveita boa parte da lógica de core_loader.rs
                              do desktop; só o transporte de carregamento
                              muda de verdade
```

Reaproveite ao máximo a lógica já escrita em `core-loader-desktop` — a
diferença real está concentrada em: transporte de carregamento de
biblioteca nativa, superfície de renderização, e input/áudio via JNI. O
`ShaderChain`/`DecorationResolver`/`SaveStateManager` não deveriam
precisar de reimplementação, só de uma superfície de I/O diferente por
baixo.

## Critério de pronto

- Um core software-only carrega e roda no Android via download dinâmico
  (não bundlado), validando que o `targetSdkVersion` baixo realmente
  contorna a restrição
- Um controle Xbox/PlayStation via Bluetooth é reconhecido automaticamente
- App instala em um dispositivo Android 14+ sem erro de instalação
  bloqueada por `targetSdkVersion`
