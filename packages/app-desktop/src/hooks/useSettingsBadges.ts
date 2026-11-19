import { useQuery } from "@tanstack/react-query";
import { listPendingMatches } from "../lib/tauri";
import { useNotificationStore } from "../stores/useNotificationStore";

export function useSettingsBadges() {
  const { data } = useQuery({
    queryKey: ["pending-matches"],
    queryFn: listPendingMatches,
    staleTime: 30_000,
    retry: false,
  });
  const notifications = useNotificationStore((state) => state.items);
  const counts = new Map<string, number>([["/settings/metadata", data?.length ?? 0]]);
  for (const item of notifications) {
    if (item.id === "update" && item.kind === "update") {
      counts.set("/settings/sistema/sobre", 1);
    } else if (item.kind === "notice" && item.route && item.count &&
      (item.id === "core-updates" || item.id === "bezel-updates" || item.id === "shader-pack-update")) {
      counts.set(item.route, item.count);
    }
  }
  return (route: string) => {
    let count = 0;
    for (const [path, items] of counts) {
      if (path === route || path.startsWith(route + "/")) count += items;
    }
    return count;
  };
}
