import { create } from "zustand";
import {
  DEFAULT_CUSTOM_HUE,
  DEFAULT_THEME_ID,
  isThemeId,
  type ThemeId,
  type ThemeMode,
  type ThemeSelection,
} from "../styles/themes";
import { getThemeSelection, setThemeSelection } from "../lib/tauri";

/**
 * Aparência: tema de cor (ver `styles/themes.ts`). A fonte da verdade é o
 * Rust (`<dados>/appearance/theme.json`, ver `appearance.rs`) — o
 * `localStorage` é por origem (dev, `tauri://` no Linux e
 * `http://tauri.localhost` no Windows têm cada um o seu) e sozinho perdia a
 * escolha entre eles. Ele segue como cache síncrono: a store nasce com o
 * valor dele (o `FluentProvider` já monta com o tema certo, sem flash) e
 * `syncFromRust` corrige logo depois se o Rust tiver outro.
 */
const KEY = "reemu.theme";

const DEFAULT_SELECTION: ThemeSelection = { kind: "preset", id: DEFAULT_THEME_ID };
const DEFAULT_DRAFT = { hue: DEFAULT_CUSTOM_HUE, mode: "dark" as ThemeMode };

interface Persisted {
  selection: ThemeSelection;
  /** Último matiz/modo escolhidos no "Personalizado" — sobrevive trocar de
   *  volta pra um preset, pra não perder o ajuste se a pessoa só espiar
   *  outro tema e voltar. */
  customDraft: { hue: number; mode: ThemeMode };
}

function isSelection(v: unknown): v is ThemeSelection {
  if (typeof v !== "object" || v === null) return false;
  const o = v as Record<string, unknown>;
  if (o.kind === "preset") return isThemeId(o.id);
  if (o.kind === "custom") {
    return typeof o.hue === "number" && (o.mode === "dark" || o.mode === "light");
  }
  return false;
}

function load(): Persisted {
  try {
    return parse(localStorage.getItem(KEY));
  } catch {
    /* modo privado / storage bloqueado */
  }
  return { selection: DEFAULT_SELECTION, customDraft: DEFAULT_DRAFT };
}

/** `null`/vazio/inválido → padrão. Aceita o formato antigo (id cru). */
function parse(raw: string | null): Persisted {
  try {
    if (!raw) return { selection: DEFAULT_SELECTION, customDraft: DEFAULT_DRAFT };
    // Formato antigo (pré-"Personalizado"): só o id do tema como string crua.
    if (isThemeId(raw)) {
      return { selection: { kind: "preset", id: raw }, customDraft: DEFAULT_DRAFT };
    }
    const parsed: unknown = JSON.parse(raw);
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      isSelection((parsed as Record<string, unknown>).selection)
    ) {
      const p = parsed as Record<string, unknown>;
      const draft = p.customDraft as Partial<Persisted["customDraft"]> | undefined;
      return {
        selection: p.selection as ThemeSelection,
        customDraft:
          draft && typeof draft.hue === "number" && (draft.mode === "dark" || draft.mode === "light")
            ? { hue: draft.hue, mode: draft.mode }
            : DEFAULT_DRAFT,
      };
    }
  } catch {
    /* JSON inválido */
  }
  return { selection: DEFAULT_SELECTION, customDraft: DEFAULT_DRAFT };
}

const inTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function persist(state: Persisted) {
  const json = JSON.stringify(state);
  try {
    localStorage.setItem(KEY, json);
  } catch {
    /* ignora — o tema ainda vale nesta sessão */
  }
  if (inTauri()) {
    setThemeSelection(json).catch(() => {
      /* sem Rust (build antigo) — fica só no localStorage */
    });
  }
}

interface ThemeState {
  selection: ThemeSelection;
  customDraft: { hue: number; mode: ThemeMode };
  setPreset: (id: ThemeId) => void;
  /** Escolhe o matiz do "Personalizado" e já ativa (mesma UX do slider do
   *  Xbox: mexer no controle troca o tema na hora). */
  setCustomHue: (hue: number) => void;
  setCustomMode: (mode: ThemeMode) => void;
  /** Ativa o "Personalizado" com o último matiz/modo lembrados. */
  activateCustom: () => void;
}

const initial = load();

export const useThemeStore = create<ThemeState>((set, get) => ({
  selection: initial.selection,
  customDraft: initial.customDraft,
  setPreset: (id) => {
    const next: Persisted = { selection: { kind: "preset", id }, customDraft: get().customDraft };
    persist(next);
    set({ selection: next.selection });
  },
  setCustomHue: (hue) => {
    const mode = get().customDraft.mode;
    const next: Persisted = { selection: { kind: "custom", hue, mode }, customDraft: { hue, mode } };
    persist(next);
    set(next);
  },
  setCustomMode: (mode) => {
    const hue = get().customDraft.hue;
    const next: Persisted = { selection: { kind: "custom", hue, mode }, customDraft: { hue, mode } };
    persist(next);
    set(next);
  },
  activateCustom: () => {
    const { hue, mode } = get().customDraft;
    const next: Persisted = { selection: { kind: "custom", hue, mode }, customDraft: { hue, mode } };
    persist(next);
    set({ selection: next.selection });
  },
}));

/**
 * Traz a escolha salva no Rust. Se o Rust ainda não tem nada (1ª vez depois
 * desta mudança), manda pra lá o que o `localStorage` tinha — quem já tinha
 * escolhido um tema não perde. Exportada pra teste; roda sozinha na carga.
 */
export async function syncFromRust(): Promise<void> {
  if (!inTauri()) return;
  let raw: string;
  try {
    raw = await getThemeSelection();
  } catch {
    return;
  }
  if (!raw) {
    let local: string | null = null;
    try {
      local = localStorage.getItem(KEY);
    } catch {
      /* storage bloqueado */
    }
    if (local) {
      const p = parse(local);
      setThemeSelection(JSON.stringify(p)).catch(() => {});
    }
    return;
  }
  const saved = parse(raw);
  const cur = useThemeStore.getState();
  if (JSON.stringify(saved) !== JSON.stringify({ selection: cur.selection, customDraft: cur.customDraft })) {
    useThemeStore.setState(saved);
    try {
      localStorage.setItem(KEY, JSON.stringify(saved));
    } catch {
      /* ignora */
    }
  }
}

void syncFromRust();
