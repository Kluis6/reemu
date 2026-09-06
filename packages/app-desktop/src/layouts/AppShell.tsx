import {
  ArrowLeftRegular,
  FullScreenMaximizeRegular,
  FullScreenMinimizeRegular,
  HomeRegular,
  LibraryRegular,
  PowerRegular,
  SearchRegular,
  SettingsRegular,
} from "@fluentui/react-icons";
import { Button, Tooltip, mergeClasses } from "@fluentui/react-components";
import { useEffect, useRef } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { ButtonHints } from "../components/ButtonHints";
import { useClock } from "../hooks/useClock";
import { useFullscreen } from "../hooks/useFullscreen";
import { quitApp } from "../lib/tauri";
import { useSearchStore } from "../stores/useSearchStore";
import { useShellStyles } from "../styles/xbox";

const RAIL = [
  { to: "/", end: true, icon: <HomeRegular />, label: "Início" },
  { to: "/library", end: true, icon: <LibraryRegular />, label: "Meus jogos" },
  {
    to: "/settings",
    end: true,
    icon: <SettingsRegular />,
    label: "Configurações",
  },
];

export function AppShell() {
  const s = useShellStyles();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const clock = useClock();
  const { on: fullscreen, toggle: toggleFullscreen } = useFullscreen();
  const atRoot = pathname === "/";
  const atBrowse = pathname === "/" || pathname === "/library";

  const search = useSearchStore();
  const searchRef = useRef<HTMLInputElement>(null);
  // Y no controle / "/" no teclado marcam `open` → foca o campo.
  useEffect(() => {
    if (search.open) searchRef.current?.focus();
  }, [search.open]);

  const hints = atBrowse
    ? ([
        { glyph: "A", label: "Selecionar" },
        { glyph: "Y", label: "Buscar" },
        { glyph: "MENU", label: "Opções" },
      ] as const)
    : ([
        { glyph: "A", label: "Selecionar" },
        { glyph: "B", label: "Voltar" },
      ] as const);

  return (
    <div className={s.app}>
      <nav className={s.rail}>
        <div className={s.railBrand} title="ReEmu">
          R
        </div>
        {RAIL.map((it) => (
          <NavLink
            key={it.to}
            to={it.to}
            end={it.end}
            className={s.railItem}
            title={it.label}
            aria-label={it.label}
          >
            {it.icon}
          </NavLink>
        ))}
        <div className={s.railSpacer} />
        <div className={s.railSep} />
        <button
          className={mergeClasses(s.railItem, s.railQuit)}
          onClick={() => void quitApp()}
          title="Fechar o ReEmu"
          aria-label="Fechar o ReEmu"
        >
          <PowerRegular />
        </button>
      </nav>

      <div className={s.main}>
        <div className={s.topbar}>
          {!atRoot && (
            <Tooltip content="Voltar para a tela anterior" relationship="label">
              <Button
                appearance="subtle"
                icon={<ArrowLeftRegular />}
                aria-label="Voltar"
                onClick={() => navigate(-1)}
              />
            </Tooltip>
          )}
          <label
            className={s.search}
            data-nav-skip
            onClick={() => searchRef.current?.focus()}
          >
            <SearchRegular />
            <input
              ref={searchRef}
              value={search.query}
              placeholder="Buscar na biblioteca…"
              onFocus={() => {
                if (pathname !== "/library") navigate("/library");
                search.setOpen(true);
              }}
              onBlur={() => search.setOpen(false)}
              onChange={(e) => search.setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  search.reset();
                  searchRef.current?.blur();
                }
              }}
            />
          </label>
          <div className={s.topbarSpacer} />
          <Tooltip
            content={
              fullscreen
                ? "Sair da tela cheia (F11)"
                : "Ocupar a tela inteira (F11)"
            }
            relationship="label"
          >
            <Button
              appearance="subtle"
              aria-label={
                fullscreen ? "Sair da tela cheia" : "Tela cheia"
              }
              icon={
                fullscreen ? (
                  <FullScreenMinimizeRegular />
                ) : (
                  <FullScreenMaximizeRegular />
                )
              }
              onClick={() => void toggleFullscreen()}
            />
          </Tooltip>
          <span className={s.clock}>{clock}</span>
        </div>
        <div className={s.scroll}>
          <Outlet />
        </div>
      </div>

      <ButtonHints hints={hints} />
    </div>
  );
}
