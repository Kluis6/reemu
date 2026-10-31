import {
  Button,
  Card,
  Dialog,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import { HeartFilled } from "@fluentui/react-icons";
import { useRef, useState, type ReactElement } from "react";
import { DIALOG_FADE_ONLY } from "../lib/motion";
import { useModalButtonStyles } from "../styles/buttons";
import { useCardStyles } from "../styles/xbox";
import { useTranslation } from "react-i18next";

export interface CardMenuItem {
  label: string;
  icon?: ReactElement;
  onClick: () => void;
}

const useLocalStyles = makeStyles({
  // Menu do jogo como nas capturas do console: centralizado, com o nome no topo.
  menuSurface: { maxWidth: "360px" },
  menuTitle: { overflowWrap: "anywhere" },
  menuList: { display: "flex", flexDirection: "column", rowGap: tokens.spacingVerticalS },
  menuItem: {
    justifyContent: "flex-start",
    columnGap: tokens.spacingHorizontalM,
    paddingTop: tokens.spacingVerticalM,
    paddingBottom: tokens.spacingVerticalM,
  },
  // Só indicador — não é botão. Sempre visível (não depende de hover),
  // já que não há mais ação de favoritar aqui (fica na tela do jogo).
  favBadge: {
    position: "absolute",
    top: "8px",
    right: "8px",
    display: "grid",
    alignItems: "center",
    justifyItems: "center",
    // Card de ~184 epx (6 por fileira, ver styles/metrics.ts): 28 com ícone
    // de 16 (múltiplo de 4, o SVG não borra) — antes 20/12, pequeno demais.
    width: "28px",
    height: "28px",
    borderRadius: tokens.borderRadiusMedium,
    // Fundo opaco na cor da página do tema e o coração no destaque do tema:
    // o par que `pickAccent` garante com 3:1 (WCAG 1.4.11) em todos os temas
    // (`themes.test.ts`). Antes era preto translúcido + tom 80, que em temas
    // de primário escuro (Crimson, Blue, Purple) quase sumia.
    backgroundColor: "var(--reemuPage)",
    color: "var(--reemuBrandAccent)",
    fontSize: "16px",
    pointerEvents: "none",
  },
  // Sem capa: o NOME do jogo no próprio card (antes eram só as iniciais —
  // "CA", "CA", "CA" não dizia qual era qual; reconhecer > lembrar).
  noArt: {
    position: "absolute",
    inset: 0,
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    gap: "6px",
    padding: "12%",
    textAlign: "center",
    // superfície de card do tema (ver `cardSurface` em styles/themes.ts):
    // antes terminava na cor da própria página e o card sumia
    backgroundImage: "linear-gradient(160deg, var(--reemuCardTop), var(--reemuCardBottom))",
    boxShadow: "inset 0 0 0 1px var(--reemuCardStroke)",
    // raio padrão do ReEmu (o mesmo da `.art` que o recorta): sem ele o traço
    // reto era cortado no arco e o canto ficava sem borda
    borderRadius: tokens.borderRadiusMedium,
  },
  noArtTitle: {
    color: tokens.colorNeutralForeground1,
    fontWeight: tokens.fontWeightRegular,
    // texto principal: mínimo de 15 epx do guia de TV
    fontSize: "15px",
    lineHeight: 1.25,
    overflowWrap: "anywhere",
    display: "-webkit-box",
    WebkitLineClamp: "3",
    WebkitBoxOrient: "vertical",
    overflow: "hidden",
  },
  noArtSub: {
    color: tokens.colorNeutralForeground3,
    fontSize: "12px",
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
  const { t } = useTranslation();
  const s = useCardStyles();
  const l = useLocalStyles();
  const [broken, setBroken] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const btn = useModalButtonStyles();
  const cardRef = useRef<HTMLDivElement>(null);
  // O foco vindo do controle não conta para a restauração do `Dialog`: ao fechar,
  // ele ficava no documento, e o controle perdia o card.
  const closeMenu = () => {
    setMenuOpen(false);
    requestAnimationFrame(() => cardRef.current?.focus({ preventScroll: true }));
  };
  const hasMenu = !!menu && menu.length > 0;
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
      data-nav-menu={hasMenu ? "" : undefined}
      ref={cardRef}
      onContextMenu={
        hasMenu
          ? (e) => {
              e.preventDefault();
              setMenuOpen(true);
            }
          : undefined
      }
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
          <span className={l.noArt}>
            <span className={l.noArtTitle}>{title}</span>
            {badge && <span className={l.noArtSub}>{badge}</span>}
          </span>
        )}
        {favorite && (
          <span className={l.favBadge} aria-label={t("shell.favorite")}>
            <HeartFilled />
          </span>
        )}
      </div>
      {/* nome sobre a capa no foco — sem capa o nome já está no card */}
      {showArt && (
        <div className={s.meta} data-meta>
          {badge && <span className={s.subText}>{badge}</span>}
          <span className={s.titleText}>{title}</span>
        </div>
      )}
    </Card>
  );

  if (!hasMenu) return card;

  return (
    <>
      {card}
      <Dialog
        open={menuOpen}
        onOpenChange={(_, d) => (d.open ? setMenuOpen(true) : closeMenu())}
        surfaceMotion={DIALOG_FADE_ONLY}
      >
        <DialogSurface className={l.menuSurface}>
          <DialogBody>
            <DialogTitle className={l.menuTitle}>{title}</DialogTitle>
            <DialogContent className={l.menuList}>
              {menu.map((m) => (
                <Button
                  key={m.label}
                  className={mergeClasses(l.menuItem, btn.secondary)}
                  icon={m.icon}
                  onClick={() => {
                    closeMenu();
                    m.onClick();
                  }}
                >
                  {m.label}
                </Button>
              ))}
            </DialogContent>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </>
  );
}
