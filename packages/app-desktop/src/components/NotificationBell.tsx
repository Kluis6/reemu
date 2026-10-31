import {
  Button,
  Caption1,
  CounterBadge,
  DrawerBody,
  DrawerHeader,
  DrawerHeaderTitle,
  OverlayDrawer,
  Tooltip,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import {
  AlertRegular,
  ArrowCircleUpRegular,
  SparkleRegular,
  WarningRegular,
} from "@fluentui/react-icons";
import { useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useNotificationStore, type AppNotification } from "../stores/useNotificationStore";
import { useTranslation } from "react-i18next";
import i18n from "../i18n";
import { TOOLTIP_POSITION } from "../lib/tooltip";
import { useModalButtonStyles } from "../styles/buttons";
import { floatingDrawer } from "../styles/xbox";

const useStyles = makeStyles({
  badge: { position: "absolute", top: "2px", right: "2px", pointerEvents: "none" },
  // Flutuante à esquerda, como o drawer de perfil e o menu de pausa.
  drawer: {
    ["--fui-Drawer--size" as string]: "min(420px, calc(100vw - 48px))",
    ...floatingDrawer("left"),
    backgroundColor: tokens.colorNeutralBackground2,
    color: tokens.colorNeutralForeground1,
  },
  // a folga em cima não deixa o corpo do drawer cortar o anel de foco
  list: {
    display: "flex",
    flexDirection: "column",
    rowGap: tokens.spacingVerticalS,
    paddingTop: "12px",
    paddingBottom: "12px",
  },
  item: {
    justifyContent: "flex-start",
    textAlign: "left",
    alignItems: "flex-start",
    columnGap: tokens.spacingHorizontalM,
    paddingTop: tokens.spacingVerticalS,
    paddingBottom: tokens.spacingVerticalS,
    width: "100%",
    maxWidth: "none",
  },
  itemText: { display: "flex", flexDirection: "column", rowGap: "2px", minWidth: 0 },
  itemTitle: { fontWeight: tokens.fontWeightSemibold },
  // ícone/indicador: destaque do tema (`reemuBrandAccent`)
  unread: { color: "var(--reemuBrandAccent)" },
  meta: { color: tokens.colorNeutralForeground3 },
  empty: {
    color: tokens.colorNeutralForeground3,
  },
});

function ago(at: number): string {
  const min = Math.round((Date.now() - at) / 60_000);
  if (min < 1) return i18n.t("notifications.now");
  if (min < 60) return i18n.t("notifications.minAgo", { count: min });
  const h = Math.round(min / 60);
  if (h < 24) return i18n.t("notifications.hAgo", { count: h });
  return new Date(at).toLocaleDateString(i18n.language);
}

/**
 * Sino da barra lateral (acima do botão Encerrar): avisos do sistema —
 * versão nova, novidades depois de atualizar, shader removido por impedir o
 * jogo de aparecer, abertos num drawer. Contador = não lidas; fechar a lista
 * marca tudo como lido. Clicar num aviso abre o modal da atualização.
 */
export function NotificationBell({ className }: { className?: string }) {
  const { t } = useTranslation();
  const s = useStyles();
  const btn = useModalButtonStyles();
  const bell = useRef<HTMLButtonElement>(null);
  const navigate = useNavigate();
  const items = useNotificationStore((n) => n.items);
  const markAllRead = useNotificationStore((n) => n.markAllRead);
  const openDialog = useNotificationStore((n) => n.openDialog);
  const openShaderDialog = useNotificationStore((n) => n.openShaderDialog);
  const [open, setOpen] = useState(false);
  const unread = items.filter((i) => !i.read).length;

  // marca ao FECHAR: enquanto a lista está aberta o destaque de "não lida"
  // continua visível. O foco vindo do controle não volta sozinho ao sino.
  const close = () => {
    setOpen(false);
    markAllRead();
    requestAnimationFrame(() => bell.current?.focus({ preventScroll: true }));
  };

  const select = (n: AppNotification) => {
    close();
    if (n.kind === "notice") {
      if (n.route) navigate(n.route);
      return;
    }
    if (n.kind === "shaderFailed") openShaderDialog(n.shader);
    else openDialog({ mode: n.kind === "update" ? "update" : "whatsNew", info: n.update });
  };

  return (
    <>
      <Tooltip positioning={TOOLTIP_POSITION} content={t("notifications.title")} relationship="label">
        <Button
          ref={bell}
          className={className}
          appearance="subtle"
          icon={<AlertRegular />}
          aria-label={unread > 0 ? t("notifications.titleUnread", { count: unread }) : t("notifications.title")}
          onClick={() => setOpen(true)}
        >
          {unread > 0 && (
            <CounterBadge
              className={s.badge}
              count={unread}
              overflowCount={9}
              size="small"
              color="brand"
            />
          )}
        </Button>
      </Tooltip>
      <OverlayDrawer
        open={open}
        onOpenChange={(_, d) => (d.open ? setOpen(true) : close())}
        position="start"
        className={s.drawer}
        aria-label={t("notifications.title")}
      >
        <DrawerHeader>
          <DrawerHeaderTitle>{t("notifications.title")}</DrawerHeaderTitle>
        </DrawerHeader>
        <DrawerBody>
          {items.length === 0 ? (
            <Caption1 className={s.empty}>{t("notifications.empty")}</Caption1>
          ) : (
            <div className={s.list}>
              {items.map((n) => {
                const iconClass = mergeClasses(!n.read && s.unread);
                return (
                  <Button
                    key={n.id}
                    className={mergeClasses(s.item, btn.secondary)}
                    icon={
                      n.kind === "update" ? (
                        <ArrowCircleUpRegular className={iconClass} />
                      ) : n.kind === "shaderFailed" || n.kind === "notice" ? (
                        <WarningRegular className={iconClass} />
                      ) : (
                        <SparkleRegular className={iconClass} />
                      )
                    }
                    onClick={() => select(n)}
                  >
                    <span className={s.itemText}>
                      <span className={mergeClasses(s.itemTitle, !n.read && s.unread)}>
                        {n.title}
                      </span>
                      <Caption1 className={s.meta}>{n.summary}</Caption1>
                      <Caption1 className={s.meta}>{ago(n.at)}</Caption1>
                    </span>
                  </Button>
                );
              })}
            </div>
          )}
        </DrawerBody>
      </OverlayDrawer>
    </>
  );
}
