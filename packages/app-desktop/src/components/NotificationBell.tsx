import {
  Button,
  Caption1,
  CounterBadge,
  Popover,
  PopoverSurface,
  PopoverTrigger,
  Subtitle2,
  Tooltip,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import {
  AlertRegular,
  ArrowCircleUpRegular,
  SparkleRegular,
} from "@fluentui/react-icons";
import { useState } from "react";
import { useNotificationStore, type AppNotification } from "../stores/useNotificationStore";

const useStyles = makeStyles({
  badge: { position: "absolute", top: "2px", right: "2px", pointerEvents: "none" },
  surface: {
    width: "340px",
    maxWidth: "calc(100vw - 32px)",
    padding: tokens.spacingVerticalM,
    display: "flex",
    flexDirection: "column",
    rowGap: tokens.spacingVerticalS,
  },
  header: { paddingLeft: tokens.spacingHorizontalS },
  list: { display: "flex", flexDirection: "column", rowGap: tokens.spacingVerticalXS },
  item: {
    justifyContent: "flex-start",
    textAlign: "left",
    alignItems: "flex-start",
    columnGap: tokens.spacingHorizontalM,
    paddingTop: tokens.spacingVerticalS,
    paddingBottom: tokens.spacingVerticalS,
    width: "100%",
    fontWeight: tokens.fontWeightRegular,
  },
  itemText: { display: "flex", flexDirection: "column", rowGap: "2px", minWidth: 0 },
  itemTitle: { fontWeight: tokens.fontWeightSemibold },
  unread: { color: tokens.colorBrandForeground1 },
  meta: { color: tokens.colorNeutralForeground3 },
  empty: {
    color: tokens.colorNeutralForeground3,
    padding: `${tokens.spacingVerticalM} ${tokens.spacingHorizontalS}`,
  },
});

function ago(at: number): string {
  const min = Math.round((Date.now() - at) / 60_000);
  if (min < 1) return "agora";
  if (min < 60) return `há ${min} min`;
  const h = Math.round(min / 60);
  if (h < 24) return `há ${h} h`;
  return new Date(at).toLocaleDateString("pt-BR");
}

/**
 * Sino da barra lateral (acima do botão Encerrar): avisos do sistema —
 * versão nova, novidades depois de atualizar. Contador = não lidas; abrir a
 * lista marca tudo como lido. Clicar num aviso abre o modal da atualização.
 */
export function NotificationBell({ className }: { className?: string }) {
  const s = useStyles();
  const items = useNotificationStore((n) => n.items);
  const markAllRead = useNotificationStore((n) => n.markAllRead);
  const openDialog = useNotificationStore((n) => n.openDialog);
  const [open, setOpen] = useState(false);
  const unread = items.filter((i) => !i.read).length;

  const select = (n: AppNotification) => {
    setOpen(false);
    openDialog({ mode: n.kind === "update" ? "update" : "whatsNew", info: n.update });
  };

  return (
    <Popover
      open={open}
      onOpenChange={(_, d) => {
        setOpen(d.open);
        // marca ao FECHAR: enquanto a lista está aberta o destaque de
        // "não lida" continua visível
        if (!d.open) markAllRead();
      }}
      positioning={{ position: "after", align: "end", offset: 12 }}
      trapFocus
    >
      <PopoverTrigger disableButtonEnhancement>
        <Tooltip content="Notificações" relationship="label">
          <Button
            className={className}
            appearance="subtle"
            icon={<AlertRegular />}
            aria-label={unread > 0 ? `Notificações — ${unread} novas` : "Notificações"}
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
      </PopoverTrigger>
      <PopoverSurface className={s.surface}>
        <Subtitle2 className={s.header}>Notificações</Subtitle2>
        {items.length === 0 ? (
          <Caption1 className={s.empty}>
            Nenhuma notificação. Avisos de versões novas do ReEmu aparecem aqui.
          </Caption1>
        ) : (
          <div className={s.list}>
            {items.map((n) => (
              <Button
                key={n.id}
                className={s.item}
                appearance="subtle"
                icon={
                  n.kind === "update" ? (
                    <ArrowCircleUpRegular className={mergeClasses(!n.read && s.unread)} />
                  ) : (
                    <SparkleRegular className={mergeClasses(!n.read && s.unread)} />
                  )
                }
                onClick={() => select(n)}
              >
                <span className={s.itemText}>
                  <span className={mergeClasses(s.itemTitle, !n.read && s.unread)}>
                    {n.title}
                  </span>
                  <Caption1 className={s.meta}>
                    {n.summary} · {ago(n.at)}
                  </Caption1>
                </span>
              </Button>
            ))}
          </div>
        )}
      </PopoverSurface>
    </Popover>
  );
}
