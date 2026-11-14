import {
  Body1,
  Button,
  Caption1,
  Menu,
  MenuItemRadio,
  MenuList,
  MenuPopover,
  MenuTrigger,
  Switch,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import { ChevronDownRegular } from "@fluentui/react-icons";
import { useEffect, useRef, useState, type ReactNode } from "react";

// Blocos de opção no modelo das configurações do console (capturas em
// docs/design/screenshots/referencia-xbox): o foco contorna o bloco inteiro, o
// valor fica à vista e a lista de um seletor abre sobre o próprio bloco.
const useStyles = makeStyles({
  group: {
    display: "flex",
    flexDirection: "column",
    rowGap: tokens.spacingVerticalM,
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(min(300px, 100%), 1fr))",
    gap: tokens.spacingHorizontalM,
  },
  block: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    columnGap: tokens.spacingHorizontalL,
    minHeight: "72px",
    boxSizing: "border-box",
    padding: `${tokens.spacingVerticalM} ${tokens.spacingHorizontalL}`,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: "var(--reemuControlFill)",
    color: tokens.colorNeutralForeground1,
    transitionProperty: "background-color",
    transitionDuration: tokens.durationFaster,
    transitionTimingFunction: tokens.curveEasyEase,
  },
  pressable: {
    cursor: "pointer",
    ":hover": { backgroundColor: "var(--reemuControlFillHover)" },
    ":active": { backgroundColor: "var(--reemuControlFillPressed)" },
  },
  // `Button` do Fluent: herda o foco e o hover do app; só o desenho é do bloco.
  selectBlock: {
    width: "100%",
    height: "100%",
    fontWeight: tokens.fontWeightRegular,
    textAlign: "left",
    border: "none",
  },
  field: {
    flexDirection: "column",
    alignItems: "stretch",
    rowGap: tokens.spacingVerticalS,
  },
  wide: { gridColumn: "1 / -1" },
  // Dentro de uma faixa de lista (Gerenciar biblioteca, Controles): mais baixo,
  // pra caber na altura da faixa sem empurrá-la.
  compact: {
    minHeight: "48px",
    padding: `${tokens.spacingVerticalXS} ${tokens.spacingHorizontalM}`,
  },
  // Sobre uma superfície da mesma cor do bloco (faixa de lista, card, dialog):
  // um tom acima, o fundo dos botões do app.
  raised: {
    backgroundColor: "var(--reemuControlFill)",
    ":hover": { backgroundColor: "var(--reemuControlFillHover)" },
    ":hover:active": { backgroundColor: "var(--reemuControlFillPressed)" },
  },
  text: {
    display: "flex",
    flexDirection: "column",
    rowGap: "2px",
    minWidth: 0,
  },
  secondary: { color: tokens.colorNeutralForeground3 },
  chevron: { flexShrink: 0, fontSize: "20px" },
  // Sem o espaço do `Switch`, que desalinhava a borda direita dos blocos.
  switch: { flexShrink: 0, "& .fui-Switch__indicator": { margin: 0 } },
  // O Fluent limita o item a 290 px: sem isso, o texto quebra num menu mais largo.
  popover: {
    maxWidth: "none",
    overflowY: "auto",
    "& [role^='menuitem']": { maxWidth: "none" },
  },
});

/** Grupo de blocos, com título opcional acima. */
export function BlockGroup({ title, children }: { title?: string; children: ReactNode }) {
  const s = useStyles();
  return (
    <section className={s.group}>
      {title && <Body1 as="h3" style={{ margin: 0 }}>{title}</Body1>}
      <div className={s.grid}>{children}</div>
    </section>
  );
}

/** Liga e desliga com efeito imediato: `Switch`, conforme o uso do Fluent 2. O
 *  `<label>` faz o bloco inteiro responder ao clique. */
export function ToggleBlock({
  label,
  description,
  checked,
  disabled,
  busy,
  onChange,
}: {
  label: string;
  description?: string;
  checked: boolean;
  disabled?: boolean;
  /** Uma ação em curso: o controle segue focável (o foco do controle não some). */
  busy?: boolean;
  onChange: (checked: boolean) => void;
}) {
  const s = useStyles();
  return (
    <label className={mergeClasses("reemu-block", s.block, !disabled && s.pressable)}>
      <span className={s.text}>
        <Body1>{label}</Body1>
        {description && <Caption1 className={s.secondary}>{description}</Caption1>}
      </span>
      <Switch
        className={s.switch}
        checked={checked}
        disabled={disabled}
        aria-busy={busy || undefined}
        onChange={(_, d) => onChange(d.checked)}
      />
    </label>
  );
}

/** Seletor com rótulo e valor em duas linhas. Usa o `Menu` com `MenuItemRadio`
 *  em vez do `Dropdown`: a navegação por controle já trata `role="menu"` como
 *  camada, e o `Dropdown` depende de setas no próprio campo. */
export function SelectBlock({
  label,
  description,
  value,
  options,
  disabled,
  onChange,
  compact,
  raised,
  className,
}: {
  label: string;
  description?: string;
  disabled?: boolean;
  value: string;
  options: readonly { value: string; label: string }[];
  onChange: (value: string) => void;
  /** Mais baixo, pra faixas de lista. */
  compact?: boolean;
  /** Fundo um tom acima, pra quando fica sobre a cor do próprio bloco. */
  raised?: boolean;
  className?: string;
}) {
  const s = useStyles();
  const [open, setOpen] = useState(false);
  const popover = useRef<HTMLDivElement>(null);
  const current = options.find((o) => o.value === value)?.label ?? "";

  // A lista abre com o item atual em foco, não com o primeiro.
  useEffect(() => {
    if (!open) return;
    const frame = requestAnimationFrame(() => {
      popover.current?.querySelector<HTMLElement>('[aria-checked="true"]')?.focus();
    });
    return () => cancelAnimationFrame(frame);
  }, [open]);

  return (
    <Menu
      open={open}
      onOpenChange={(_, d) => setOpen(d.open)}
      checkedValues={{ value: [value] }}
      onCheckedValueChange={(_, d) => {
        const next = d.checkedItems[0];
        if (next !== undefined) onChange(next);
      }}
      // `autoSize`: lista longa de um core rola por dentro, sem passar da janela
      positioning={{
        position: "below",
        align: "start",
        coverTarget: true,
        matchTargetSize: "width",
        autoSize: "height",
      }}
    >
      <MenuTrigger disableButtonEnhancement>
        <Button
          appearance="transparent"
          className={mergeClasses(s.block, s.pressable, s.selectBlock, compact && s.compact, raised && s.raised, className)}
          aria-label={`${label}: ${current}`}
          disabled={disabled}
          aria-description={description}
        >
          <span className={s.text}>
            <Body1>{label}</Body1>
            <Caption1 className={s.secondary}>{current}</Caption1>
          </span>
          <ChevronDownRegular className={s.chevron} />
        </Button>
      </MenuTrigger>
      <MenuPopover ref={popover} className={s.popover}>
        <MenuList>
          {options.map((o) => (
            <MenuItemRadio key={o.value} name="value" value={o.value}>
              {o.label}
            </MenuItemRadio>
          ))}
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}

/** Bloco para o que não é liga/desliga nem lista, como campos de texto e sliders.
 *  `focusBlock`: o foco contorna o bloco, como no slider das capturas do console. */
export function FieldBlock({
  wide,
  focusBlock,
  children,
}: {
  wide?: boolean;
  focusBlock?: boolean;
  children: ReactNode;
}) {
  const s = useStyles();
  return (
    <div className={mergeClasses(focusBlock && "reemu-block", s.block, s.field, wide && s.wide)}>
      {children}
    </div>
  );
}
