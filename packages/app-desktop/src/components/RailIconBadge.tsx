import { CounterBadge, makeStyles } from "@fluentui/react-components";
import type { ReactNode } from "react";

const useStyles = makeStyles({
  icon: { position: "relative", display: "inline-flex" },
  badge: {
    position: "absolute",
    bottom: 0,
    right: 0,
    transform: "translate(25%, 25%)",
    pointerEvents: "none",
  },
});

export function RailIconBadge({
  children,
  count,
  overflowCount = 9,
}: {
  children: ReactNode;
  count: number;
  overflowCount?: number;
}) {
  const s = useStyles();
  return (
    <span className={s.icon} aria-hidden="true">
      {children}
      {count > 0 && (
        <CounterBadge className={s.badge} count={count} overflowCount={overflowCount} size="small" color="brand" />
      )}
    </span>
  );
}
