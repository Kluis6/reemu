import {
  makeStyles,
  Menu,
  MenuItem,
  MenuList,
  MenuPopover,
  MenuTrigger,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import { StarFilled, StarRegular } from "@fluentui/react-icons";
import { useState } from "react";
import { initials } from "../lib/initials";
import { useCardStyles } from "../styles/xbox";

const useLocalStyles = makeStyles({
  // revela a estrela no hover/foco do cartão
  reveal: {
    "&:hover [data-fav], &:focus-within [data-fav]": { opacity: 1 },
  },
  fav: {
    position: "absolute",
    top: "6px",
    right: "6px",
    width: "26px",
    height: "26px",
    display: "grid",
    placeItems: "center",
    borderRadius: "999px",
    cursor: "pointer",
    color: tokens.colorNeutralForegroundInverted,
    backgroundColor: "rgba(0, 0, 0, 0.5)",
    opacity: 0,
    transitionProperty: "opacity, color, background-color",
    transitionDuration: "140ms",
    ":hover": { backgroundColor: "rgba(0, 0, 0, 0.72)" },
    "&[data-on]": {
      opacity: 1,
      color: tokens.colorPaletteMarigoldForeground1,
    },
  },
});

export interface CardMenuItem {
  label: string;
  onClick: () => void;
}

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

  const card = (
    <button className={mergeClasses(s.card, l.reveal)} onClick={onClick}>
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
            role="button"
            tabIndex={0}
            aria-label={favorite ? "Desfavoritar" : "Favoritar"}
            aria-pressed={favorite}
            className={l.fav}
            data-fav=""
            data-on={favorite ? "" : undefined}
            onClick={(e) => {
              e.stopPropagation();
              onToggleFavorite();
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                e.stopPropagation();
                onToggleFavorite();
              }
            }}
          >
            {favorite ? <StarFilled /> : <StarRegular />}
          </span>
        )}
      </div>
    </button>
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
