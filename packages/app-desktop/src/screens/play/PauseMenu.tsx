import {
  Badge,
  Body1,
  Button,
  DrawerBody,
  DrawerFooter,
  DrawerHeader,
  DrawerHeaderTitle,
  OverlayDrawer,
  Tab,
  TabList,
  Tooltip,
  mergeClasses,
} from "@fluentui/react-components";
import {
  ArrowExitRegular,
  DeleteRegular,
  FullScreenMaximizeRegular,
  FullScreenMinimizeRegular,
  HistoryRegular,
  PlayRegular,
  SaveRegular,
  TrophyRegular,
  WandRegular,
  WarningRegular,
} from "@fluentui/react-icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { CheatsPanel } from "../../components/CheatsPanel";
import { SaveStateThumb } from "../../components/SaveStateThumb";
import { RemoteStateList } from "../../components/sync/RemoteStateList";
import { useFullscreen } from "../../hooks/useFullscreen";
import { REEMU_TAB } from "../../hooks/useMenuNav";
import { focusFromNav, moveFocus } from "../../lib/focusNav";
import { formatPlayTime } from "../../lib/playTime";
import { errorToast, sysToast } from "../../lib/toast";
import { TOOLTIP_POSITION } from "../../lib/tooltip";
import { deleteSaveState, listSaveStates, loadSaveState, saveState } from "../../lib/tauri";
import { useGamepadStore } from "../../stores/useGamepadStore";
import { useToastStore } from "../../stores/useToastStore";
import { useModalButtonStyles } from "../../styles/buttons";
import { usePauseStyles } from "../../styles/xbox";

/**
 * Menu de pausa num drawer do Fluent à esquerda (pedido do usuário,
 * 2026-10-29; guia de uso do Drawer no Fluent 2: cabeçalho com título, corpo
 * que rola, rodapé com as ações; navegação à esquerda). O Esc NÃO fecha por
 * aqui: o backend já alterna o menu com ele (`commands/input.rs`) — fechar
 * pelos dois reabria. Clicar fora retoma o jogo, como o Continuar.
 */
export function PauseMenu({
  open,
  romId,
  onResume,
  onQuit,
}: {
  open: boolean;
  romId: string;
  onResume: () => void;
  onQuit: () => void;
}) {
  const { t, i18n } = useTranslation();
  const pause = usePauseStyles();
  const btn = useModalButtonStyles();
  const qc = useQueryClient();
  const push = useToastStore((s) => s.push);
  const padConnected = useGamepadStore((st) => st.devices.length > 0);
  const { on: fullscreen, toggle: toggleFullscreen } = useFullscreen();

  // Abas só ícone, como o guia do Xbox (print do usuário, 2026-10-29); volta
  // pra 1ª a cada vez que o menu abre. Continuar, tela cheia e sair ficam fora
  // das abas, no rodapé do painel. LB/RB trocam (`REEMU_TAB`). Cheats só com
  // jogo da biblioteca (precisa do id).
  const tabs = useMemo(
    () => [
      { id: "states" as const, label: "play.tabStates" as const, icon: <SaveRegular /> },
      ...(romId
        ? [{ id: "cheats" as const, label: "play.cheats" as const, icon: <WandRegular /> }]
        : []),
      // RetroAchievements (doc 16): por enquanto só o aviso de que vem aí
      { id: "achievements" as const, label: "play.achievements" as const, icon: <TrophyRegular /> },
    ],
    [romId],
  );
  type PauseTab = (typeof tabs)[number]["id"];
  const [tab, setTab] = useState<PauseTab>("states");
  // ajuste no render quando o menu abre (react.dev, "Adjusting some state
  // when a prop changes"), sem efeito
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) setTab("states");
  }

  const bodyRef = useRef<HTMLDivElement>(null);
  // Trocou de aba pelo atalho: o foco vai pro 1º item da aba nova, como no
  // Xbox (o clique na aba deixa o foco nela).
  const focusBody = useRef(false);
  useEffect(() => {
    if (!open) return;
    const onTab = (e: Event) => {
      const step = (e as CustomEvent<number>).detail;
      const ids = tabs.map((x) => x.id);
      setTab((cur) => ids[(ids.indexOf(cur) + step + ids.length) % ids.length]);
      focusBody.current = true;
    };
    window.addEventListener(REEMU_TAB, onTab);
    return () => window.removeEventListener(REEMU_TAB, onTab);
  }, [open, tabs]);
  useEffect(() => {
    if (!focusBody.current) return;
    focusBody.current = false;
    const first = bodyRef.current?.querySelector<HTMLElement>(
      "button:not([disabled]), input:not([disabled])",
    );
    if (first) focusFromNav(first);
  }, [tab]);

  // A navegação pelo gamepad é global (`useMenuNav` no RootLayout). Aqui só
  // focamos o 1º item ao abrir, pra `confirm` ter alvo.
  useEffect(() => {
    if (!open) return;
    const raf = requestAnimationFrame(() => moveFocus("down"));
    return () => cancelAnimationFrame(raf);
  }, [open]);

  const quickSave = useMutation({
    mutationFn: () => saveState(romId, 0),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["save-states", romId] });
      push(sysToast(t("play.quickSaved"), "Success"));
    },
    onError: (e) => push(errorToast(e, "saveGameState")),
  });
  const quickLoad = useMutation({
    mutationFn: async () => {
      const list = await listSaveStates(romId);
      const quick = list.find((s) => s.slot === 0);
      if (!quick) throw new Error("nenhum QuickSave pra este jogo");
      await loadSaveState(quick.id);
    },
    onSuccess: () => push(sysToast(t("play.quickLoaded"), "Success")),
    onError: (e) => push(errorToast(e, "loadGameState")),
  });
  // Lista de save states — só busca com o menu aberto.
  const stateList = useQuery({
    queryKey: ["save-states", romId],
    queryFn: () => listSaveStates(romId),
    enabled: open,
    retry: false,
  });
  const loadAny = useMutation({
    mutationFn: (id: string) => loadSaveState(id),
    onSuccess: () => {
      push(sysToast(t("play.stateLoaded"), "Success"));
      onResume();
    },
    onError: (e) => push(errorToast(e, "loadGameState")),
  });
  const delState = useMutation({
    mutationFn: (id: string) => deleteSaveState(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["save-states", romId] }),
    onError: (e) => push(errorToast(e, "deleteSaveState")),
  });

  return (
    <OverlayDrawer
      open={open}
      position="start"
      size="small"
      className={pause.drawer}
      aria-label={t("play.paused")}
      onOpenChange={(_, d) => {
        if (!d.open && d.type === "backdropClick") onResume();
      }}
    >
      <DrawerHeader className={pause.header}>
        <div className={pause.tabsRow}>
          {/* nome do botão do controle, igual em todo idioma; sem controle não
              há atalho e a dica some */}
          {padConnected && (
            <kbd className={pause.shoulder} aria-hidden>
              LB
            </kbd>
          )}
          <TabList
            className={pause.tabs}
            size="large"
            aria-label={t("play.tabs")}
            selectedValue={tab}
            onTabSelect={(_, d) => setTab(d.value as PauseTab)}
          >
            {tabs.map((x) => (
              <Tooltip positioning={TOOLTIP_POSITION} key={x.id} content={t(x.label)} relationship="label">
                <Tab value={x.id} icon={x.icon} />
              </Tooltip>
            ))}
          </TabList>
          {padConnected && (
            <kbd className={pause.shoulder} aria-hidden>
              RB
            </kbd>
          )}
        </div>
        <DrawerHeaderTitle heading={{ as: "h2", className: pause.title }}>
          {t(tabs.find((x) => x.id === tab)?.label ?? "play.tabStates")}
        </DrawerHeaderTitle>
      </DrawerHeader>

      <DrawerBody className={pause.drawerBody}>
        <div ref={bodyRef} className={pause.body} role="tabpanel">
          {tab === "states" && (
            <>
              <Button
                size="large"
                appearance="subtle"
                className={pause.item}
                icon={<SaveRegular />}
                disabled={quickSave.isPending}
                onClick={() => quickSave.mutate()}
              >
                {t("play.quickSave")}
              </Button>
              <Button
                size="large"
                appearance="subtle"
                className={pause.item}
                icon={<HistoryRegular />}
                disabled={quickLoad.isPending}
                onClick={() => quickLoad.mutate()}
              >
                {t("play.quickLoad")}
              </Button>
              {(stateList.data?.length ?? 0) > 0 && (
                <div className={pause.states}>
                  {stateList.data!.map((st) => (
                    <div key={st.id} className={pause.stateRow}>
                      <SaveStateThumb stateId={st.id} hasThumbnail={st.hasThumbnail} />
                      <div className={pause.stateInfo}>
                        <span className={pause.stateLabel}>
                          {st.slot === 0
                            ? t("play.quickSave")
                            : st.slot != null
                              ? t("game.slot", { n: st.slot })
                              : t("game.auto")}
                          <br />
                          <span className={pause.stateDate}>
                            {new Date(st.createdAt * 1000).toLocaleString(i18n.language)}
                            {st.playTimeAtSave != null &&
                              t("game.playedFor", { time: formatPlayTime(st.playTimeAtSave) })}
                          </span>
                          {/* gravado com outra versão do core: avisa antes de carregar */}
                          {st.versionChanged && (
                            <>
                              <br />
                              <Badge appearance="tint" color="warning" icon={<WarningRegular />}>
                                {t("game.otherVersion")}
                              </Badge>
                            </>
                          )}
                        </span>
                        <div className={pause.stateActions}>
                          <Button
                            appearance="subtle"
                            icon={<HistoryRegular />}
                            disabled={loadAny.isPending}
                            onClick={() => loadAny.mutate(st.id)}
                          >
                            {t("play.loadState")}
                          </Button>
                          <Button
                            appearance="subtle"
                            icon={<DeleteRegular />}
                            disabled={delState.isPending}
                            onClick={() => delState.mutate(st.id)}
                          >
                            {t("common.delete")}
                          </Button>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
              {/* states de outras máquinas: baixa e carrega no core aberto */}
              <RemoteStateList
                romId={romId}
                coreId={null}
                onDownloaded={async (saved) => {
                  qc.invalidateQueries({ queryKey: ["save-states", romId] });
                  await loadAny.mutateAsync(saved.id).catch(() => {});
                }}
              />
            </>
          )}

          {tab === "cheats" && romId && <CheatsPanel romId={romId} />}

          {tab === "achievements" && (
            <div className={pause.soon}>
              <TrophyRegular className={pause.soonIcon} aria-hidden />
              <Body1>{t("play.achievementsSoon")}</Body1>
            </div>
          )}
        </div>
      </DrawerBody>

      {/* Ações do jogo, só ícone, no pé do painel como a fileira de baixo do
          guia do Xbox; o nome vem no tooltip (Fluent 2: ícone sozinho pede
          rótulo acessível e tooltip). */}
      <DrawerFooter className={pause.footer}>
        {[
          { label: t("play.continue"), icon: <PlayRegular />, onClick: onResume },
          {
            label: fullscreen ? t("shell.exitFullscreen") : t("shell.fullscreen"),
            icon: fullscreen ? <FullScreenMinimizeRegular /> : <FullScreenMaximizeRegular />,
            onClick: () => void toggleFullscreen(),
          },
          { label: t("play.quit"), icon: <ArrowExitRegular />, onClick: onQuit },
        ].map((a) => (
          <Tooltip key={a.label} content={a.label} relationship="label" positioning="above">
            <Button
              size="large"
              className={mergeClasses(pause.iconAction, btn.secondary)}
              icon={a.icon}
              onClick={a.onClick}
            />
          </Tooltip>
        ))}
      </DrawerFooter>
    </OverlayDrawer>
  );
}
