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
import { HeartFilled } from "@fluentui/react-icons";
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
    // Fluido: o card (`gameCardSize` em xbox.ts) cresce de 150 até 445px —
    // um badge de canto fixo em 24px virava um pontinho perdido num card
    // gigante em 4K. `1.25vw` bate ~24px em 1920px (mesmo tamanho de hoje),
    // teto moderado (não escala 1:1 com o card, só o suficiente pra não
    // sumir).
    width: "clamp(20px, 1.25vw, 42px)",
    height: "clamp(20px, 1.25vw, 42px)",
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: "rgba(0, 0, 0, 0.45)",
    // Mesma cor de marca do coração da página do jogo (RomDetail) —
    // `--reemuBrandText`, não `colorBrandForeground1` (falha contraste AA
    // no tema claro, ver `styles/themes.ts::ReEmuTokens.reemuBrandText`).
    color: "var(--reemuBrandText)",
    fontSize: "clamp(12px, 0.7vw, 24px)",
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
            // `sync`: força o decode completo do bitmap ANTES do primeiro
            // paint — sem isto, o `onLoad` (que só garante os bytes
            // baixados) podia disparar antes do raster estar pronto, e a
            // imagem aparecia com um pedaço (geralmente o topo) ainda não
            // decodificado por um instante, mesmo já com opacity:1. Capas
            // remotas grandes (Mega Drive: PNGs de ~500-800KB) são as mais
            // afetadas.
            decoding="sync"
            data-loaded={loaded ? "" : undefined}
            onLoad={() => setLoaded(true)}
            onError={() => setBroken(true)}
          />
        ) : (
          <span style={{ fontSize: 30, opacity: 0.5 }}>{initials(title)}</span>
        )}
        {favorite && (
          <span className={l.favBadge} aria-label="Favorito">
            <HeartFilled />
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
