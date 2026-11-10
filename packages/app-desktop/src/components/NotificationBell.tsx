import {
  Body1,
  Button,
  Caption1,
  CounterBadge,
  DrawerBody,
  DrawerHeader,
  DrawerHeaderTitle,
  OverlayDrawer,
  Switch,
  Tooltip,
  makeStyles,
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
import { floatingDrawer } from "../styles/xbox";
import { CloseButton } from "./CloseButton";

const useStyles = makeStyles({
  // O sino no centro do botão e o badge no canto de baixo à direita DO SINO,
  // por cima dele (pedido do usuário, 2026-11-10). O badge ia como filho do `Button`: o
  // Fluent tratava o botão como "ícone + texto" e empurrava o sino pro lado,
  // e o badge ficava no canto do botão. Agora os dois vão no slot do ícone.
  bellIcon: { position: "relative", display: "inline-flex" },
  badge: {
    position: "absolute",
    bottom: 0,
    right: 0,
    // como os badges da barra lateral do VS Code: o badge cobre o quarto de
    // baixo à direita do ícone e passa só 4 px da borda dele
    transform: "translate(25%, 25%)",
    pointerEvents: "none",
  },
  // Flutuante à esquerda, como o drawer de perfil e o menu de pausa.
  drawer: {
    ["--fui-Drawer--size" as string]: "min(420px, calc(100vw - 48px))",
    ...floatingDrawer("left"),
    backgroundColor: tokens.colorNeutralBackground2,
    color: tokens.colorNeutralForeground1,
  },
  // Cabeçalho com a mesma margem do corpo (24): o X termina na borda dos cards.
  // Laterais do drawer de perfil: 10 na lista (o respiro de 16 fica dentro de
  // cada item). O cabeçalho acompanha: o título começa onde começa o texto do
  // filtro e o ícone dos itens (10 + 16), e o X termina na borda deles (10).
  // O Fluent puxa o botão de ação 8 px pra fora; aqui ele para na margem.
  header: {
    paddingLeft: "26px",
    paddingRight: "10px",
    "& .fui-DrawerHeaderTitle__action": { marginRight: 0 },
  },
  body: { paddingLeft: "10px", paddingRight: "10px" },
  // Filtro "só não lidas" numa faixa preenchida, como no app Xbox.
  filter: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    columnGap: tokens.spacingHorizontalM,
    marginTop: "8px",
    padding: "6px 4px 6px 16px",
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground3,
  },
  // Folga em cima: o anel e o brilho do foco não encostam no filtro.
  list: {
    display: "flex",
    flexDirection: "column",
    rowGap: "4px",
    paddingTop: "20px",
    paddingBottom: "12px",
  },
  // Linha: imagem quadrada, título e texto, e o tempo à direita. Sem borda e
  // sem fundo: a cor só aparece no hover e no foco, como no app Xbox.
  item: {
    display: "grid",
    gridTemplateColumns: "48px minmax(0, 1fr) auto",
    alignItems: "start",
    columnGap: "16px",
    width: "100%",
    maxWidth: "none",
    minHeight: "64px",
    padding: "12px 16px",
    textAlign: "left",
    fontWeight: tokens.fontWeightRegular,
    borderTopColor: "transparent",
    borderRightColor: "transparent",
    borderBottomColor: "transparent",
    borderLeftColor: "transparent",
    backgroundColor: "transparent",
    ":focus-visible, :global(html[data-nav-focus]) &:focus": {
      backgroundColor: tokens.colorNeutralBackground3,
    },
    ":hover, :hover:active": {
      backgroundColor: tokens.colorNeutralBackground3Hover,
      borderTopColor: "transparent",
      borderRightColor: "transparent",
      borderBottomColor: "transparent",
      borderLeftColor: "transparent",
    },
  },
  // ícone num quadrado na cor de destaque do tema, como o troféu do Xbox
  thumb: {
    display: "grid",
    placeItems: "center",
    width: "48px",
    height: "48px",
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorBrandBackground,
    color: tokens.colorNeutralForegroundOnBrand,
    fontSize: "26px",
  },
  itemText: { display: "flex", flexDirection: "column", rowGap: "2px", minWidth: 0 },
  itemTitle: { fontWeight: tokens.fontWeightSemibold, color: tokens.colorNeutralForeground1 },
  summary: { color: tokens.colorNeutralForeground2 },
  time: { color: tokens.colorNeutralForeground3, whiteSpace: "nowrap", paddingTop: "2px" },
  empty: {
    color: tokens.colorNeutralForeground3,
    paddingTop: "20px",
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
  const bell = useRef<HTMLButtonElement>(null);
  const navigate = useNavigate();
  const items = useNotificationStore((n) => n.items);
  const markAllRead = useNotificationStore((n) => n.markAllRead);
  const openDialog = useNotificationStore((n) => n.openDialog);
  const openShaderDialog = useNotificationStore((n) => n.openShaderDialog);
  const [open, setOpen] = useState(false);
  const [onlyUnread, setOnlyUnread] = useState(false);
  const unread = items.filter((i) => !i.read).length;
  const shown = onlyUnread ? items.filter((i) => !i.read) : items;

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
          icon={
            <span className={s.bellIcon}>
              <AlertRegular />
              {unread > 0 && (
                <CounterBadge
                  className={s.badge}
                  count={unread}
                  overflowCount={9}
                  size="small"
                  color="brand"
                />
              )}
            </span>
          }
          aria-label={unread > 0 ? t("notifications.titleUnread", { count: unread }) : t("notifications.title")}
          onClick={() => setOpen(true)}
        />
      </Tooltip>
      <OverlayDrawer
        open={open}
        onOpenChange={(_, d) => (d.open ? setOpen(true) : close())}
        position="start"
        className={s.drawer}
        aria-label={t("notifications.title")}
      >
        <DrawerHeader className={s.header}>
          <DrawerHeaderTitle
            action={<CloseButton onClick={close} />}
          >
            {t("notifications.title")}
          </DrawerHeaderTitle>
        </DrawerHeader>
        <DrawerBody className={s.body}>
          <label className={s.filter}>
            <Body1>{t("notifications.onlyUnread")}</Body1>
            <Switch checked={onlyUnread} onChange={(_, d) => setOnlyUnread(d.checked)} />
          </label>
          {shown.length === 0 ? (
            <Caption1 className={s.empty}>
              {items.length === 0 ? t("notifications.empty") : t("notifications.noUnread")}
            </Caption1>
          ) : (
            <div className={s.list}>
              {shown.map((n) => (
                <Button
                  key={n.id}
                  className={s.item}
                  onClick={() => select(n)}
                >
                  <span className={s.thumb} aria-hidden>
                    {n.kind === "update" ? (
                      <ArrowCircleUpRegular />
                    ) : n.kind === "shaderFailed" || n.kind === "notice" ? (
                      <WarningRegular />
                    ) : (
                      <SparkleRegular />
                    )}
                  </span>
                  <span className={s.itemText}>
                    <span className={s.itemTitle}>{n.title}</span>
                    <Caption1 className={s.summary}>{n.summary}</Caption1>
                  </span>
                  <Caption1 className={s.time}>{ago(n.at)}</Caption1>
                </Button>
              ))}
            </div>
          )}
        </DrawerBody>
      </OverlayDrawer>
    </>
  );
}
