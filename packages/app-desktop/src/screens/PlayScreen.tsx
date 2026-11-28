import { RewindRegular } from "@fluentui/react-icons";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useState, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate, useParams } from "react-router-dom";
import { ConflictDialog } from "../components/sync/ConflictDialog";
import { useFocusBridge } from "../hooks/useFocusBridge";
import { useKeyboardInput } from "../hooks/useKeyboardInput";
import {
  nativeVideoActive,
  nativeVideoOverlay,
  onRewindState,
  toggleFocus,
  unloadGame,
} from "../lib/tauri";
import { useFocusStore } from "../stores/useFocusStore";
import { LaunchError, LaunchSplash, NoTarget } from "./play/LaunchScreens";
import { PauseMenu } from "./play/PauseMenu";
import { usePlayStyles } from "./play/styles";
import { useCanvasVideo } from "./play/useCanvasVideo";
import { useGameSession, useQuickStateToasts } from "./play/useGameSession";
import { useLaunchInfo } from "./play/useLaunchInfo";
import { usePauseBackground } from "./play/usePauseBackground";

/**
 * Tela de jogo (`/play/:romId`). As partes ficam em `screens/play/`: o jogo a
 * abrir (`useLaunchInfo`), carregar e descarregar (`useGameSession`), o vídeo
 * no canvas (`useCanvasVideo`), o fundo do menu de pausa no vídeo nativo
 * (`usePauseBackground`), o menu de pausa (`PauseMenu`) e as telas de abertura
 * e de erro (`LaunchScreens`).
 */
export function PlayScreen() {
  const { t } = useTranslation();
  const styles = usePlayStyles();
  const navigate = useNavigate();
  const [rewinding, setRewinding] = useState(false);
  // Vídeo numa surface nativa atrás da webview → esta tela fica transparente e
  // não roda o loop do canvas.
  const nativeVideo =
    useQuery({
      queryKey: ["native-video"],
      queryFn: nativeVideoActive,
      staleTime: Infinity,
      retry: false,
    }).data === true;
  // Protótipo REEMU_WIN_OVERLAY: o jogo continua visível atrás da interface,
  // então o menu de pausa dispensa o print de fundo.
  const overlay =
    useQuery({
      queryKey: ["native-video-overlay"],
      queryFn: nativeVideoOverlay,
      staleTime: Infinity,
      retry: false,
    }).data === true;
  const { romId = "" } = useParams();
  // "Voltar" = tela anterior do histórico. Só cai num destino fixo se não
  // houver histórico (reload direto na URL /play/...).
  const goBack = () => {
    if (window.history.length > 1) navigate(-1);
    else navigate(romId ? `/rom/${romId}` : "/");
  };

  useKeyboardInput();
  useFocusBridge();
  const focus = useFocusStore((s) => s.focus);
  const setFocus = useFocusStore((s) => s.setFocus);

  const { launch, loading: launchLoading } = useLaunchInfo(romId);
  const { status, aspect, setAspect, declaredAspect, answerConflict } = useGameSession(launch, romId);
  useQuickStateToasts(romId);
  const { canvasRef, decoCanvasRef, deco } = useCanvasVideo({
    running: status === "ready",
    nativeVideo,
    declaredAspect,
    setAspect,
  });

  // Surface nativa: o jogo é apresentado pelo Rust direto na subsurface; deixa
  // html/body/#root transparentes pra ela aparecer atrás da webview.
  useEffect(() => {
    if (!nativeVideo) return;
    document.documentElement.classList.add("native-video");
    return () => document.documentElement.classList.remove("native-video");
  }, [nativeVideo]);

  useEffect(() => {
    let off: (() => void) | undefined;
    let alive = true;
    void onRewindState((active) => {
      if (alive) setRewinding(active);
    }).then((fn) => {
      if (alive) off = fn;
      else fn();
    });
    return () => {
      alive = false;
      off?.();
    };
  }, []);

  const menuOpen = focus === "MenuFocused";
  const pauseBg = usePauseBackground(menuOpen, nativeVideo, overlay);

  const resume = () => {
    void toggleFocus()
      .then(setFocus)
      .catch(() => setFocus("GameFocused"));
  };
  const quit = () => {
    void unloadGame().catch(() => {});
    // Único caminho até aqui é o botão "Jogar" do RomDetail (push simples,
    // sem `replace`) — volta pra lá em vez de sempre pro Início.
    navigate(-1);
  };

  if (!launch && !launchLoading) return <NoTarget onBack={goBack} />;
  if (status === "loading") return <LaunchSplash launch={launch} />;
  // A checagem da nuvem achou o save deste jogo mudado nas duas máquinas
  // (D118): o mesmo diálogo da página do jogo, por cima da abertura. Um lado
  // ou "Decidir depois" abre o jogo; fechar desiste e volta. Sem jogo
  // rodando, o controle navega no diálogo (`data-launch-ask`, useMenuNav).
  if (typeof status === "object" && "conflict" in status)
    return (
      <div data-launch-ask>
        <LaunchSplash launch={launch} />
        <ConflictDialog
          conflict={status.conflict}
          onDone={(outcome) => (outcome === "dismiss" ? goBack() : answerConflict(outcome))}
        />
      </div>
    );
  if (typeof status === "object") return <LaunchError error={status.error} launch={launch} onBack={goBack} />;

  return (
    <div className={styles.root} style={nativeVideo ? { background: "transparent" } : undefined}>
      {!nativeVideo && (
        <div
          className={deco ? styles.decoStage : styles.plainStage}
          style={
            deco
              ? ({ ["--reemuDecoAr" as string]: String(deco.w / deco.h) } as CSSProperties)
              : undefined
          }
        >
          <canvas
            ref={canvasRef}
            className={deco ? styles.gameInDeco : styles.canvas}
            width={256}
            height={240}
            style={deco ? undefined : { aspectRatio: String(aspect) }}
          />
          {deco && <canvas ref={decoCanvasRef} className={styles.decoLayer} aria-hidden />}
        </div>
      )}

      {/* Vídeo nativo: a janela do jogo some enquanto o menu está aberto e a
          página é transparente — sem uma camada opaca JÁ no 1º quadro, o
          fundo branco da WebView aparecia até o escurecido do drawer
          (animado, 250 ms) e o print chegarem ("tela pisca em branco",
          2026-10-29). Preto na hora; o print do jogo entra por cima. */}
      {pauseBg.mounted && nativeVideo && !overlay && (
        <div className={styles.pauseLayer} aria-hidden>
          {pauseBg.bg && <img className={styles.pauseBg} src={pauseBg.bg} alt="" />}
        </div>
      )}
      {rewinding && !menuOpen && (
        <div className={styles.rewindBadge} role="status" aria-live="polite">
          <RewindRegular />
          <span>{t("play.rewinding")}</span>
        </div>
      )}
      <PauseMenu open={menuOpen} romId={romId} onResume={resume} onQuit={quit} />
    </div>
  );
}
