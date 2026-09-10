import {
  Button,
  Card,
  Menu,
  MenuItem,
  MenuList,
  MenuPopover,
  MenuTrigger,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import { StarFilled, StarRegular } from "@fluentui/react-icons";
import { useState } from "react";
import { initials } from "../lib/initials";
import { useCardStyles } from "../styles/xbox";

export interface CardMenuItem {
  label: string;
  onClick: () => void;
}

const useLocalStyles = makeStyles({
  // revela a estrela no hover/foco do cartão
  reveal: {
    "&:hover [data-fav], &:focus-within [data-fav]": { opacity: 1 },
  },
  // wrapper de posicionamento — só posição/opacidade; o botão fica com o
  // border-radius padrão do Fluent.
  favSlot: {
    position: "absolute",
    top: "5px",
    right: "5px",
    opacity: 0,
    transitionProperty: "opacity",
    transitionDuration: "140ms",
    "&[data-on]": { opacity: 1 },
  },
  favBtn: {
    color: tokens.colorNeutralForegroundInverted,
    backgroundColor: "rgba(0, 0, 0, 0.45)",
    ":hover": {
      color: tokens.colorNeutralForegroundInverted,
      backgroundColor: "rgba(0, 0, 0, 0.7)",
    },
  },
  favOn: { color: tokens.colorPaletteMarigoldForeground1 },
});

/**
 * Cartão de jogo no estilo Xbox: tile quadrado, badge de plataforma, estrela
 * de favorito. Botão / clique-direito abre o menu de contexto (`menu`).
 */
export function GameCard({
  title,
  badge,
  boxart,
  favorite,
  onToggleFavorite,
  onClick,
  menu,
}: {
  title: string;
  subtitle?: string;
  badge?: string;
  boxart?: string | null;
  favorite?: boolean;
  onToggleFavorite?: () => void;
  onClick?: () => void;
  menu?: readonly CardMenuItem[];
}) {
  const s = useCardStyles();
  const l = useLocalStyles();
  const [broken, setBroken] = useState(false);
  const showArt = boxart && !broken;

  // `<Card>` do Fluent com `onClick` já vira focável, mas NÃO ganha
  // `role`/teclado — a gente adiciona (a nav por controle e o leitor de tela
  // tratam o tile inteiro como botão). A estrela é um `<Button>` aninhado com
  // `stopPropagation`.
  const card = (
    <Card
      className={mergeClasses(s.card, l.reveal)}
      role="button"
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick?.();
        }
      }}
    >
      <div className={s.art} data-art>
        {showArt ? (
          <img
            src={boxart}
            alt={title}
            loading="lazy"
            onError={() => setBroken(true)}
          />
        ) : (
          <span style={{ fontSize: 30, opacity: 0.5 }}>{initials(title)}</span>
        )}
        {badge && <span className={s.badge}>{badge}</span>}
        {onToggleFavorite && (
          <span
            className={l.favSlot}
            data-fav=""
            data-on={favorite ? "" : undefined}
          >
            <Button
              size="small"
              appearance="subtle"
              className={mergeClasses(l.favBtn, favorite && l.favOn)}
              aria-label={favorite ? "Desfavoritar" : "Favoritar"}
              aria-pressed={favorite}
              icon={favorite ? <StarFilled /> : <StarRegular />}
              onClick={(e) => {
                e.stopPropagation();
                onToggleFavorite();
              }}
            />
          </span>
        )}
      </div>
    </Card>
  );

  if (!menu || menu.length === 0) return card;

  return (
    <Menu openOnContext>
      <MenuTrigger disableButtonEnhancement>{card}</MenuTrigger>
      <MenuPopover>
        <MenuList>
          {menu.map((m) => (
            <MenuItem key={m.label} onClick={m.onClick}>
              {m.label}
            </MenuItem>
          ))}
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}
