/**
 * O `<DialogSurface>` do Fluent v9 anima a entrada/saída escalando a
 * propriedade CSS `scale` isolada via Web Animations API. Nesta janela
 * (WebKitGTK sem aceleração de compositing na NVIDIA — ver memória
 * `frontend-perf-webkitgtk`) isso não interpola suave: o modal "salta" de um
 * estado pro outro em vez de crescer. `outScale`/`inScale` iguais a 1 zeram
 * só o efeito de escala — o fade de opacidade (que anima bem aqui) continua,
 * então a entrada/saída segue suave, só sem o "zoom".
 *
 * Passar em `surfaceMotion` de todo `<DialogSurface>` do app:
 * `<DialogSurface surfaceMotion={DIALOG_FADE_ONLY}>`.
 */
export const DIALOG_FADE_ONLY = { outScale: 1, inScale: 1 } as const;
