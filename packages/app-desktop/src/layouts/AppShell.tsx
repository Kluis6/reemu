import {
  ChevronLeftRegular,
  FullScreenMaximizeRegular,
  FullScreenMinimizeRegular,
  HomeRegular,
  LibraryRegular,
  PeopleRegular,
  PersonRegular,
  PowerRegular,
  PresenceAvailableRegular,
  SettingsRegular,
  SignOutRegular,
  TrophyRegular,
} from "@fluentui/react-icons";
import {
  Button,
  Menu,
  MenuDivider,
  MenuItem,
  MenuList,
  MenuPopover,
  MenuTrigger,
  SearchBox,
  Tooltip,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { AnimatedBackground } from "../components/AnimatedBackground";
import { ButtonHints } from "../components/ButtonHints";
import { ProfileAvatar } from "../components/ProfileAvatar";
import { RouteTransition } from "../components/RouteTransition";
import { useClock } from "../hooks/useClock";
import { useFullscreen } from "../hooks/useFullscreen";
import { useSmoothScroll } from "../hooks/useSmoothScroll";
import { getProfile, quitApp } from "../lib/tauri";
import { useSearchStore } from "../stores/useSearchStore";
import { useShellStyles } from "../styles/xbox";

const useLocalStyles = makeStyles({
  // ícones da sidebar com o border-radius padrão do botão do Fluent
  railRadius: { borderRadius: tokens.borderRadiusMedium },
  // botões de ícone da topbar: fundo igual ao item ativo da sidebar
  surface: {
    backgroundColor: "var(--reemuSurfaceSoft)",
    color: tokens.colorNeutralForeground1,
    ":hover": {
      backgroundColor: "var(--reemuSurfaceSoft)",
      color: tokens.colorNeutralForeground1,
    },
  },
});

// Presença/status do perfil ainda não existe — o badge e o anel do avatar
// entram quando tivermos rede social. Por ora, sempre desligados.
const SHOW_AVATAR_BADGE = false;
const SHOW_AVATAR_RING = false;

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

// Menu do avatar. Conquistas/rede ainda não têm tela — ficam desabilitados.
// "Sair" fecha o app.
const PROFILE_EXTRA = [
  { icon: <TrophyRegular />, label: "Minhas conquistas" },
  { icon: <PeopleRegular />, label: "Minha rede" },
] as const;

export function AppShell() {
  const s = useShellStyles();
  const l = useLocalStyles();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const clock = useClock();
  const { on: fullscreen, toggle: toggleFullscreen } = useFullscreen();
  const atRoot = pathname === "/";
  const atBrowse = pathname === "/" || pathname === "/library";

  const profile = useQuery({ queryKey: ["profile"], queryFn: getProfile, retry: false });
  const search = useSearchStore();
  const searchRef = useRef<HTMLInputElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  useSmoothScroll(scrollRef);
  // Y no controle / "/" no teclado marcam `open` → foca o campo.
  useEffect(() => {
    if (search.open) searchRef.current?.focus();
  }, [search.open]);
  // volta ao topo ao trocar de tela (a animação de entrada cobre o salto)
  useEffect(() => {
    if (scrollRef.current) scrollRef.current.scrollTop = 0;
  }, [pathname]);

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
      <AnimatedBackground />
      <nav className={s.rail}>
        <Menu positioning={{ position: "after", align: "top", offset: 8 }}>
          <MenuTrigger disableButtonEnhancement>
            <button className={s.railBrand} aria-label="Perfil" type="button">
              <ProfileAvatar
                profile={
                  profile.data ?? { name: "Jogador", avatar: "preset:1" }
                }
                size={32}
                ring={SHOW_AVATAR_RING}
                badge={SHOW_AVATAR_BADGE ? "available" : undefined}
              />
            </button>
          </MenuTrigger>
          <MenuPopover>
            <MenuList hasIcons>
              <MenuItem
                icon={<PersonRegular />}
                onClick={() => navigate("/settings/perfil")}
              >
                {profile.data?.name ? `Perfil — ${profile.data.name}` : "Meu perfil"}
              </MenuItem>
              {PROFILE_EXTRA.map((m) => (
                <MenuItem key={m.label} icon={m.icon} disabled>
                  {m.label}
                </MenuItem>
              ))}
              <MenuDivider />
              <Menu>
                <MenuTrigger disableButtonEnhancement>
                  <MenuItem icon={<PresenceAvailableRegular />} disabled>
                    Status
                  </MenuItem>
                </MenuTrigger>
                <MenuPopover>
                  <MenuList>
                    <MenuItem>Online</MenuItem>
                    <MenuItem>Ausente</MenuItem>
                    <MenuItem>Jogando</MenuItem>
                  </MenuList>
                </MenuPopover>
              </Menu>
              <MenuDivider />
              <MenuItem icon={<SignOutRegular />} onClick={() => void quitApp()}>
                Sair
              </MenuItem>
            </MenuList>
          </MenuPopover>
        </Menu>

        {RAIL.map((it) => (
          <NavLink
            key={it.to}
            to={it.to}
            end={it.end}
            className={mergeClasses(s.railItem, l.railRadius)}
            title={it.label}
            aria-label={it.label}
          >
            {it.icon}
          </NavLink>
        ))}
        <div className={s.railSpacer} />
        <div className={s.railSep} />

        <Tooltip content="Fechar o ReEmu" relationship="label">
          <Button
            className={mergeClasses(s.railItem, s.railQuit, l.railRadius)}
            onClick={() => void quitApp()}
            aria-label="Fechar o ReEmu"
            appearance="subtle"
            icon={<PowerRegular />}
          />
        </Tooltip>
      </nav>

      <div className={s.main}>
        <div className={s.topbar}>
          {!atRoot && (
            <Tooltip content="Voltar para a tela anterior" relationship="label">
              <Button
                appearance="subtle"
                className={l.surface}
                icon={<ChevronLeftRegular />}
                aria-label="Voltar"
                onClick={() => navigate(-1)}
              />
            </Tooltip>
          )}
          <SearchBox
            ref={searchRef}
            className={s.search}
            data-nav-skip
            appearance="filled-darker"
            value={search.query}
            placeholder="Buscar na biblioteca…"
            onFocus={() => {
              if (pathname !== "/library") navigate("/library");
              search.setOpen(true);
            }}
            onBlur={() => search.setOpen(false)}
            onChange={(_, d) => search.setQuery(d.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                search.reset();
                searchRef.current?.blur();
              }
            }}
          />
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
              className={l.surface}
              aria-label={fullscreen ? "Sair da tela cheia" : "Tela cheia"}
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
        <div className={s.scroll} ref={scrollRef}>
          <RouteTransition
            routeKey={pathname.startsWith("/settings") ? "/settings" : pathname}
          >
            <Outlet />
          </RouteTransition>
        </div>
      </div>

      <ButtonHints hints={hints} />
    </div>
  );
}
