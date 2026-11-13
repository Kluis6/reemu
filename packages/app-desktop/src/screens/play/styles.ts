import { makeStyles, tokens } from "@fluentui/react-components";

/** Estilos da tela de jogo (`PlayScreen` e as partes em `screens/play/`). */
export const usePlayStyles = makeStyles({
  // Opaco: no modo canvas o frame vai num <canvas>; no vídeo nativo (subsurface)
  // a área fica transparente e o jogo aparece atrás (ver src-tauri/src/video.rs).
  root: {
    position: "fixed",
    inset: 0,
    background: "#000",
    display: "grid",
    placeItems: "center",
  },
  // Fundo do menu de pausa no vídeo nativo: o frame que estava na tela,
  // borrado e escurecido por animação (a subsurface do jogo some).
  // Camada atrás do menu de pausa no vídeo nativo: opaca desde o 1º quadro,
  // por cima do jogo e abaixo do drawer (o portal do Fluent fica em
  // z-index 1000000).
  pauseLayer: {
    position: "fixed",
    inset: 0,
    zIndex: 99,
    backgroundColor: "#000",
  },
  pauseBg: {
    position: "absolute",
    inset: 0,
    objectFit: "cover",
    width: "100%",
    height: "100%",
    // blur/brightness estáticos (animar o raio do blur re-rasteriza a cada
    // frame — pesado no WebKitGTK); só a opacity anima.
    filter: "blur(9px) brightness(0.42)",
    animationName: { from: { opacity: 0 }, to: { opacity: 1 } },
    animationDuration: tokens.durationNormal,
    animationTimingFunction: tokens.curveDecelerateMid,
    animationFillMode: "forwards",
    "@media (prefers-reduced-motion: reduce)": { animationName: "none" },
  },
  menuIn: {
    animationName: {
      from: { opacity: 0, transform: "translateY(10px) scale(0.985)" },
      to: { opacity: 1, transform: "none" },
    },
    animationDuration: tokens.durationFast,
    animationTimingFunction: tokens.curveDecelerateMax,
    animationFillMode: "both",
    "@media (prefers-reduced-motion: reduce)": { animationName: "none" },
  },
  menuOut: {
    animationName: {
      from: { opacity: 1, transform: "none" },
      to: { opacity: 0, transform: "translateY(6px)" },
    },
    // saída mais curta que a entrada, acelerando
    animationDuration: tokens.durationFaster,
    animationTimingFunction: tokens.curveAccelerateMid,
    animationFillMode: "both",
    "@media (prefers-reduced-motion: reduce)": { animationName: "none" },
  },
  // Sem moldura o palco não gera caixa: o canvas se comporta como filho
  // direto do `root` (centralizado pelo grid).
  plainStage: { display: "contents" },
  // Com moldura: palco na proporção da moldura, o maior que couber na
  // janela. O jogo fica no retângulo que o Rust calcula (`game_rect`) e a
  // moldura por cima — mesma imagem que a composição na GPU daria, sem
  // mandar 8 MB por quadro pelo IPC.
  decoStage: {
    position: "relative",
    width: "min(100vw, calc(100vh * var(--reemuDecoAr)))",
    aspectRatio: "var(--reemuDecoAr)",
    overflow: "hidden",
    background: "#000",
  },
  gameInDeco: {
    position: "absolute",
    imageRendering: "pixelated",
  },
  decoLayer: {
    position: "absolute",
    inset: 0,
    width: "100%",
    height: "100%",
    pointerEvents: "none",
  },
  canvas: {
    // Preenche a altura da janela mantendo a proporção; encolhe se ficar
    // mais largo que a janela. `aspectRatio` vem do core (inline).
    height: "100%",
    width: "auto",
    maxWidth: "100%",
    maxHeight: "100%",
    imageRendering: "pixelated",
    background: "#000",
  },
  errorBox: {
    textAlign: "center",
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    gap: "12px",
    width: "min(520px, calc(100vw - 48px))",
    boxSizing: "border-box",
    padding: tokens.spacingHorizontalXXL,
    // painel como os menus: o erro não fica solto sobre o fundo
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground2,
    color: tokens.colorNeutralForeground1,
  },
  errorGame: { color: tokens.colorNeutralForeground3 },
  errorActions: {
    display: "flex",
    flexWrap: "wrap",
    justifyContent: "center",
    gap: "8px",
    marginTop: "4px",
  },
  errorDetails: {
    marginTop: "8px",
    maxWidth: "100%",
    color: tokens.colorNeutralForeground3,
    fontSize: tokens.fontSizeBase200,
    // `Accordion` do Fluent no lugar do `<details>` do HTML: mesmo estilo,
    // estados e foco dos outros recolhíveis do app (ex.: licenças).
    "& code": {
      display: "block",
      marginTop: "6px",
      textAlign: "left",
      whiteSpace: "pre-wrap",
      overflowWrap: "anywhere",
    },
  },
  center: {
    position: "fixed",
    inset: 0,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    background: "var(--reemuPage)",
  },
  splash: {
    position: "fixed",
    inset: 0,
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    gap: tokens.spacingVerticalL,
    // linear (não radial multicamada) — ver histórico do WebKitGTK em main.rs.
    background: "var(--reemuAppBg)",
    color: tokens.colorNeutralForeground1,
  },
  splashArt: {
    width: "208px",
    height: "208px",
    borderRadius: tokens.borderRadiusXLarge,
    objectFit: "cover",
    display: "grid",
    placeItems: "center",
    fontSize: "52px",
    fontWeight: 700,
    color: tokens.colorNeutralForeground4,
    background: tokens.colorNeutralBackground4,
    // Fluent 2 elevation (não um box-shadow cru): shadow16 (a tier de
    // "callouts/hover cards") — pega o peso visual do card grande e
    // isolado, e acompanha o tema (a opacidade some sozinha muda entre
    // claro/escuro, ao contrário de um rgba(0,0,0,..) fixo).
    boxShadow: tokens.shadow16,
  },
  splashTitle: {
    fontFamily: "var(--reemuFontDisplay)",
    fontSize: "22px",
    fontWeight: 700,
    textAlign: "center",
    maxWidth: "70%",
  },
  // fluent2.microsoft.design/typography: "use sentence case, nunca all caps".
  splashSub: {
    opacity: 0.55,
    fontSize: "13px",
    letterSpacing: "0.02em",
  },
});
