import { makeStyles, mergeClasses, tokens } from "@fluentui/react-components";
import { initials } from "../lib/initials";
import { useCardStyles } from "../styles/xbox";

const useStyles = makeStyles({
  tile: {
    width: "100%",
    aspectRatio: "1 / 1",
    display: "block",
    border: "none",
    padding: 0,
    cursor: "pointer",
    color: "inherit",
  },
  // 2×2 de capas, gap pequeno, cantos como os outros cards
  grid: {
    width: "100%",
    height: "100%",
    borderRadius: tokens.borderRadiusLarge,
    overflow: "hidden",
    display: "grid",
    gridTemplateColumns: "1fr 1fr",
    gridTemplateRows: "1fr 1fr",
    gap: "3px",
    backgroundColor: tokens.colorNeutralBackground1,
    padding: "3px",
    boxShadow: "0 6px 14px rgba(0, 0, 0, 0.22)",
  },
  cell: {
    minWidth: 0,
    minHeight: 0,
    display: "grid",
    placeItems: "center",
    overflow: "hidden",
    borderRadius: tokens.borderRadiusSmall,
    backgroundColor: tokens.colorNeutralBackground4,
    "& img": { width: "100%", height: "100%", objectFit: "cover" },
  },
  init: { fontSize: "13px", opacity: 0.45 },
});

type Mini = { id: string; title: string; boxart?: string | null };

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
    <button
      className={mergeClasses(c.card, s.tile)}
      aria-label={ariaLabel}
      onClick={onClick}
    >
      <div className={s.grid}>
        {cells.map((m) => (
          <div key={m.id} className={s.cell}>
            {m.boxart ? (
              <img src={m.boxart} alt="" loading="lazy" />
            ) : (
              <span className={s.init}>{initials(m.title)}</span>
            )}
          </div>
        ))}
        {Array.from({ length: Math.max(0, 4 - cells.length) }).map((_, i) => (
          <div key={`e${i}`} className={s.cell} />
        ))}
      </div>
    </button>
  );
}
