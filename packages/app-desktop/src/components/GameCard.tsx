import {
  Card,
  Menu,
  MenuItem,
  MenuList,
  MenuPopover,
  MenuTrigger,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { StarFilled } from "@fluentui/react-icons";
import { useState } from "react";
import { initials } from "../lib/initials";
import { useCardStyles } from "../styles/xbox";

export interface CardMenuItem {
  label: string;
  onClick: () => void;
}

const useLocalStyles = makeStyles({
  // Só indicador — não é botão. Sempre visível (não depende de hover),
  // já que não há mais ação de favoritar aqui (fica na tela do jogo).
  favBadge: {
    position: "absolute",
    top: "5px",
    right: "5px",
    display: "grid",
    alignItems: "center",
    justifyItems: "center",
    width: "24px",
    height: "24px",
    borderRadius: tokens.borderRadiusCircular,
    backgroundColor: "rgba(0, 0, 0, 0.45)",
    color: tokens.colorPaletteMarigoldForeground1,
    fontSize: "14px",
    pointerEvents: "none",
  },
});

/**
 * Cartão de jogo no estilo modo XBOX: o card É a capa quadrada. O nome (+
 * plataforma, `badge`) fica SOBRE a imagem e só aparece no hover/foco. A
 * estrela de favorito é só indicador (aparece quando `favorite` já é
 * `true`) — favoritar/desfavoritar é ação da tela de detalhes do jogo, não
 * do cartão. Botão / clique-direito abre o menu de contexto.
 */
export function GameCard({
  title,
  badge,
  boxart,
  favorite,
  onClick,
  menu,
}: {
  title: string;
  /** Linha secundária embaixo do título (normalmente a plataforma). */
  badge?: string;
  boxart?: string | null;
  favorite?: boolean;
  onClick?: () => void;
  menu?: readonly CardMenuItem[];
}) {
  const s = useCardStyles();
  const l = useLocalStyles();
  const [broken, setBroken] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const showArt = boxart && !broken;

  // `<Card>` do Fluent com `onClick` já vira focável, mas NÃO ganha
  // `role`/teclado — a gente adiciona (a nav por controle e o leitor de tela
  // tratam o tile inteiro como botão).
  const card = (
    <Card
      className={s.card}
      appearance="filled"
      role="button"
      aria-label={title}
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
            alt=""
            loading="lazy"
            data-loaded={loaded ? "" : undefined}
            onLoad={() => setLoaded(true)}
            onError={() => setBroken(true)}
          />
        ) : (
          <span style={{ fontSize: 30, opacity: 0.5 }}>{initials(title)}</span>
        )}
        {favorite && (
          <span className={l.favBadge} aria-label="Favorito">
            <StarFilled />
          </span>
        )}
      </div>
      <div className={s.meta} data-meta>
        {badge && <span className={s.subText}>{badge}</span>}
        <span className={s.titleText}>{title}</span>
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
