import { makeStyles, mergeClasses, tokens } from "@fluentui/react-components";
import { GridDotsRegular } from "@fluentui/react-icons";
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
    textAlign: "left",
  },
  grid: {
    position: "relative",
    width: "100%",
    height: "100%",
    borderRadius: tokens.borderRadiusLarge,
    overflow: "hidden",
    display: "grid",
    gridTemplateColumns: "1fr 1fr",
    gridTemplateRows: "1fr 1fr",
    gap: "2px",
    backgroundColor: tokens.colorNeutralBackground3,
    boxShadow: "0 6px 14px rgba(0, 0, 0, 0.22)",
  },
  cell: {
    position: "relative",
    minWidth: 0,
    minHeight: 0,
    display: "grid",
    placeItems: "center",
    backgroundColor: tokens.colorNeutralBackground4,
    overflow: "hidden",
    "& img": { width: "100%", height: "100%", objectFit: "cover" },
  },
  init: { fontSize: "14px", opacity: 0.5 },
  overlay: {
    position: "absolute",
    inset: 0,
    display: "flex",
    flexDirection: "column",
    justifyContent: "flex-end",
    padding: "8px",
    gap: "3px",
    background:
      "linear-gradient(to top, rgba(0,0,0,0.78) 0%, rgba(0,0,0,0.12) 55%, transparent 100%)",
    color: tokens.colorNeutralForeground1,
  },
  label: { fontSize: "12px", fontWeight: 700, display: "flex", alignItems: "center", gap: "5px" },
  sub: { fontSize: "11px", opacity: 0.85 },
});

type Mini = { id: string; title: string; boxart?: string | null };

/**
 * "Card de 4" no fim da prateleira de uma plataforma: 2×2 de capas + rótulo
 * "ver todos". Clicar leva pra `/library/<systemId>`.
 */
export function PlatformTile({
  label,
  total,
  sample,
  onClick,
}: {
  label: string;
  total: number;
  sample: readonly Mini[];
  onClick: () => void;
}) {
  const s = useStyles();
  const c = useCardStyles();
  const cells = sample.slice(0, 4);
  return (
    <button className={mergeClasses(c.card, s.tile)} onClick={onClick}>
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
        <div className={s.overlay}>
          <span className={s.label}>
            <GridDotsRegular /> {label}
          </span>
          <span className={s.sub}>Ver todos os {total}</span>
        </div>
      </div>
    </button>
  );
}
