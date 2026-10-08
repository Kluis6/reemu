import {
  DesktopRegular,
  DeveloperBoardRegular,
  KeyboardRegular,
  LibraryRegular,
  PaintBrushRegular,
  PersonRegular,
  PuzzlePieceRegular,
  Speaker2Regular,
  TagMultipleRegular,
  TvRegular,
  XboxControllerRegular,
} from "@fluentui/react-icons";
import type { FluentIcon } from "@fluentui/react-icons";

// Categorias das Configurações: o hub (`SettingsHome`) mostra uma por card e o
// `SettingsLayout` usa o nome no caminho do topo. `key` = trecho da rota;
// `id` = chave de tradução em `settings.tabs.*` e `settings.desc.*`.
export const SETTINGS_SECTIONS = [
  { key: "perfil", id: "profile", Icon: PersonRegular },
  { key: "aparencia", id: "appearance", Icon: PaintBrushRegular },
  { key: "biblioteca", id: "library", Icon: LibraryRegular },
  { key: "audio", id: "audio", Icon: Speaker2Regular },
  { key: "video", id: "video", Icon: TvRegular },
  { key: "metadata", id: "metadata", Icon: TagMultipleRegular },
  { key: "hotkeys", id: "hotkeys", Icon: KeyboardRegular },
  { key: "controllers", id: "controllers", Icon: XboxControllerRegular },
  { key: "cores", id: "cores", Icon: DeveloperBoardRegular },
  { key: "bios", id: "bios", Icon: PuzzlePieceRegular },
  { key: "sistema", id: "system", Icon: DesktopRegular },
] as const satisfies readonly {
  key: string;
  id: string;
  Icon: FluentIcon;
}[];

export type SettingsSection = (typeof SETTINGS_SECTIONS)[number];
