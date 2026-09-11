import { Button, makeStyles, mergeClasses, tokens } from "@fluentui/react-components";
import { useState } from "react";
import { initials } from "../lib/initials";
import { useCardStyles } from "../styles/xbox";

const useStyles = makeStyles({
  // Mesma caixa quadrada do GameCard (mergeClasses com `c.card` garante as
  // mesmas proporções) — só troca o conteúdo por um grid 2×2.
  tile: {
    width: "100%",
    minWidth: 0,
    aspectRatio: "1 / 1",
    display: "block",
    border: "none",
    padding: 0,
    cursor: "pointer",
    color: "inherit",
  },
  // 2×2 de capas: padding em volta do grupo + gap entre as células.
  grid: {
    width: "100%",
    height: "100%",
    borderRadius: tokens.borderRadiusLarge,
    overflowX: "hidden",
    overflowY: "hidden",
    display: "grid",
    gridTemplateColumns: "1fr 1fr",
    gridTemplateRows: "1fr 1fr",
    gap: "6px",
    backgroundColor: tokens.colorNeutralBackground1,
    padding: "6px",
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
  const showArt = item.boxart && !broken;
  return (
    <div className={cellClassName}>
      {showArt ? (
        <img
          src={item.boxart ?? undefined}
          alt=""
          loading="lazy"
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
    <Button
      appearance="transparent"
      className={mergeClasses(c.card, s.tile)}
      aria-label={ariaLabel}
      onClick={onClick}
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
    </Button>
  );
}
