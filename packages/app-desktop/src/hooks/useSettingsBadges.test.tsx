// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { useSettingsBadges } from "./useSettingsBadges";
import { SETTINGS_SECTIONS } from "../layouts/settingsSections";
import { useNotificationStore } from "../stores/useNotificationStore";

vi.mock("../lib/tauri", () => ({ listPendingMatches: () => Promise.resolve([]) }));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

it("counts a category once while retaining every item's count, even after reading the notices", async () => {
  const qc = new QueryClient();
  qc.setQueryData(["pending-matches"], Array.from({ length: 12 }));
  const { upsert, markAllRead } = useNotificationStore.getState();
  for (const [id, route, count] of [
    ["core-updates", "/settings/cores", 7],
    ["bezel-updates", "/settings/aparencia/decoracoes/molduras", 4],
    ["shader-pack-update", "/settings/aparencia/decoracoes/shaders", 1],
    ["other", "/settings/sincronizacao", 100],
  ] as const) {
    upsert({ id, route, count, kind: "notice", title: "", summary: "", at: 1, read: false });
  }
  markAllRead();
  const host = document.createElement("div");
  const root = createRoot(host);
  function Counts() {
    const countFor = useSettingsBadges();
    return <>{[
      SETTINGS_SECTIONS.filter(({ key }) => countFor("/settings/" + key) > 0).length,
      countFor("/settings/metadata"),
      countFor("/settings/cores"),
      countFor("/settings/aparencia"),
      countFor("/settings/aparencia/decoracoes"),
      countFor("/settings/aparencia/decoracoes/molduras"),
      countFor("/settings/aparencia/decoracoes/shaders"),
    ].join(",")}</>;
  }
  try {
    await act(async () => root.render(<QueryClientProvider client={qc}><Counts /></QueryClientProvider>));
    expect(host.textContent).toBe("3,12,7,5,5,4,1");
  } finally {
    await act(async () => root.unmount());
    qc.clear();
    useNotificationStore.setState({ items: [] });
  }
});
