import { Card, makeStyles, mergeClasses, tokens } from "@fluentui/react-components";
import { useState } from "react";
import { initials } from "../lib/initials";
import { useCardStyles } from "../styles/xbox";

// Padding do grupo 2×2 = gap entre as células — mesmo valor nos dois (um
// só reaproveitado nas duas propriedades), fluido com a tela em vez de
// fixo (mesma convenção de `clamp()` do resto do card, ver `xbox.ts`).
const TILE_GAP = "clamp(4px, 0.5vw, 16px)";

const useStyles = makeStyles({
  // `c.card` (mergeClasses) já cobre proporção/borda/padding/cursor — só
  // falta `minWidth: 0` (a caixa fica dentro de um grid, então sem isso o
  // conteúdo interno podia forçar a célula a crescer além da vaga).
  tile: {
    minWidth: 0,
  },
  // 2×2 de capas (2 colunas × 2 linhas, 4 células centralizadas): padding
  // responsivo em volta do grupo + gap entre as células, mesmo valor.
  grid: {
    width: "100%",
    height: "100%",
    borderRadius: tokens.borderRadiusLarge,
    overflowX: "hidden",
    overflowY: "hidden",
    display: "grid",
    gridTemplateColumns: "1fr 1fr",
    gridTemplateRows: "1fr 1fr",
    gap: TILE_GAP,
    backgroundColor: tokens.colorNeutralBackground1,
    padding: TILE_GAP,
  },
  cell: {
    minWidth: 0,
    minHeight: 0,
    display: "grid",
    placeItems: "center",
    overflowX: "hidden",
    overflowY: "hidden",
    borderRadius: tokens.borderRadiusSmall,
    backgroundColor: tokens.colorNeutralBackground4,
    "& img": {
      width: "100%",
      height: "100%",
      objectFit: "cover",
      objectPosition: "center",
      // some até o onLoad — mesmo motivo do GameCard (evita mostrar a
      // imagem ainda decodificando).
      opacity: 0,
      transitionProperty: "opacity",
      transitionDuration: "220ms",
      "&[data-loaded]": { opacity: 1 },
    },
  },
  init: { fontSize: "13px", opacity: 0.45 },
});

type Mini = { id: string; title: string; boxart?: string | null };

/** Uma célula do 2×2 — cai pras iniciais se a capa não carregar (URL de
 *  thumbnail remota que 404, por exemplo), em vez de deixar o ícone de
 *  imagem quebrada do navegador preso na tela. */
function TileCell({
  item,
  cellClassName,
  initClassName,
}: {
  item: Mini;
  cellClassName: string;
  initClassName: string;
}) {
  const [broken, setBroken] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const showArt = item.boxart && !broken;
  return (
    <div className={cellClassName}>
      {showArt ? (
        <img
          src={item.boxart ?? undefined}
          alt=""
          loading="lazy"
          // Ver GameCard.tsx — decode síncrono evita a capa aparecer com um
          // pedaço ainda não decodificado logo após o `onLoad`.
          decoding="sync"
          data-loaded={loaded ? "" : undefined}
          onLoad={() => setLoaded(true)}
          onError={() => setBroken(true)}
        />
      ) : (
        <span className={initClassName}>{initials(item.title)}</span>
      )}
    </div>
  );
}

/**
 * "Card de 4" no fim da prateleira de uma plataforma: 2×2 de capas, sem
 * texto (estilo "Jump back in" do Xbox). Clicar leva pra `/library/<systemId>`.
 * Mesmo `<Card>` do Fluent que o `GameCard` (não `<Button>`) — é dali que
 * vem o anel de hover/foco, pra ficar idêntico ao resto da prateleira.
 */
export function PlatformTile({
  sample,
  onClick,
  ariaLabel,
}: {
  sample: readonly Mini[];
  onClick: () => void;
  ariaLabel: string;
}) {
  const s = useStyles();
  const c = useCardStyles();
  const cells = sample.slice(0, 4);
  return (
    <Card
      className={mergeClasses(c.card, s.tile)}
      appearance="filled"
      role="button"
      aria-label={ariaLabel}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
    >
      <div className={s.grid}>
        {cells.map((m) => (
          <TileCell
            key={m.id}
            item={m}
            cellClassName={s.cell}
            initClassName={s.init}
          />
        ))}
        {Array.from({ length: Math.max(0, 4 - cells.length) }).map((_, i) => (
          <div key={`e${i}`} className={s.cell} />
        ))}
      </div>
    </Card>
  );
}
