import {
  ChevronLeftRegular,
  FullScreenMaximizeRegular,
  FullScreenMinimizeRegular,
  HomeFilled,
  HomeRegular,
  LibraryFilled,
  LibraryRegular,
  PeopleRegular,
  PersonRegular,
  PowerRegular,
  SettingsFilled,
  SettingsRegular,
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
import { useEffect, useRef, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { AnimatedBackground } from "../components/AnimatedBackground";
import { ButtonHints } from "../components/ButtonHints";
import { Clock } from "../components/Clock";
import { PowerMenuDialog } from "../components/PowerMenuDialog";
import { ProfileAvatar } from "../components/ProfileAvatar";
import { RouteTransition } from "../components/RouteTransition";
import { useFullscreen } from "../hooks/useFullscreen";
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
  // Menu do avatar — proporções do menu do modo XBOX de verdade: mais
  // largo, com respiro no CORPO da lista (mesmo valor nos 4 lados — o
  // `<MenuList>` do Fluent vem com padding horizontal menor que o
  // vertical por padrão), não em cada item — as linhas continuam com o
  // padding "medium" padrão do Fluent.
  menuPopover: { minWidth: "236px" },
  menuBody: { padding: tokens.spacingVerticalS },
  // O <MenuDivider> do Fluent vem com só 4px de margem vertical — some
  // fácil entre linhas com o padding maior do body. `!important`: o
  // componente injeta a própria classe (`margin: 4px -5px 4px -5px`)
  // depois da nossa.
  menuDivider: {
    margin: `${tokens.spacingVerticalS} -5px !important`,
  },
});

// Presença/status do perfil ainda não existe — o badge e o anel do avatar
// entram quando tivermos rede social. Por ora, sempre desligados.
const SHOW_AVATAR_BADGE = false;
const SHOW_AVATAR_RING = false;

// Ícone Regular normalmente, Filled quando a rota está ativa — mesma
// linguagem do rail do Xbox de verdade.
const RAIL = [
  {
    to: "/",
    end: true,
    icon: <HomeRegular />,
    activeIcon: <HomeFilled />,
    label: "Início",
  },
  {
    to: "/library",
    end: true,
    icon: <LibraryRegular />,
    activeIcon: <LibraryFilled />,
    label: "Meus jogos",
  },
  {
    to: "/settings",
    // NÃO `end: true`: "/settings" sozinho nunca é a rota renderizada — o
    // índice redireciona pra "/settings/perfil" (ver router.tsx), então com
    // `end` o NavLink nunca batia e o ícone nunca acendia em nenhuma
    // subpágina de Configurações.
    end: false,
    icon: <SettingsRegular />,
    activeIcon: <SettingsFilled />,
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
  const { on: fullscreen, toggle: toggleFullscreen } = useFullscreen();
  const atRoot = pathname === "/";
  const atBrowse = pathname === "/" || pathname === "/library";
  const [powerOpen, setPowerOpen] = useState(false);

  const profile = useQuery({
    queryKey: ["profile"],
    queryFn: getProfile,
    retry: false,
  });
  const search = useSearchStore();
  const searchRef = useRef<HTMLInputElement>(null);
  // Rolagem por wheel é NATIVA (o WebKitGTK já entrega inércia/suavidade
  // sozinho) — um `wheel` handler com `preventDefault` pra suavizar via JS
  // tira o scroll da thread rápida e joga na main thread, que é onde
  // React/estilo competem: fica MAIS lento, não mais suave (ver
  // trac.webkit.org/changeset/270425 e web.dev/articles/animations-guide).
  // `scroll-behavior: smooth` no CSS cobre o scrollIntoView do gamepad.
  const scrollRef = useRef<HTMLDivElement>(null);
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
        <Menu positioning={{ position: "below", align: "start", offset: 12 }}>
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
          <MenuPopover className={l.menuPopover}>
            {/* SEM `hasIcons`: só os itens que passam `icon` (Perfil,
                conquistas, rede) ficam indentados — Status/Sair, sem
                `icon`, ficam alinhados à esquerda. */}
            <MenuList className={l.menuBody}>
              <MenuItem
                icon={<PersonRegular />}
                onClick={() => navigate("/settings/perfil")}
              >
                Meu perfil
              </MenuItem>
              {PROFILE_EXTRA.map((m) => (
                <MenuItem key={m.label} icon={m.icon} disabled>
                  {m.label}
                </MenuItem>
              ))}
              <MenuDivider />
              <Menu>
                <MenuTrigger disableButtonEnhancement>
                  <MenuItem disabled>Status</MenuItem>
                </MenuTrigger>
                <MenuPopover className={l.menuPopover}>
                  <MenuList className={l.menuBody}>
                    <MenuItem>Online</MenuItem>
                    <MenuItem>Ausente</MenuItem>
                    <MenuItem>Jogando</MenuItem>
                  </MenuList>
                </MenuPopover>
              </Menu>

              <MenuItem onClick={() => void quitApp()}>Sair</MenuItem>
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
            {({ isActive }) => (isActive ? it.activeIcon : it.icon)}
          </NavLink>
        ))}
        <div className={s.railSpacer} />
        <div className={s.railSep} />

        <Tooltip content="Encerrar" relationship="label">
          <Button
            className={mergeClasses(s.railItem, s.railQuit, l.railRadius)}
            onClick={() => setPowerOpen(true)}
            aria-label="Encerrar"
            appearance="subtle"
            icon={<PowerRegular />}
          />
        </Tooltip>
      </nav>

      <PowerMenuDialog open={powerOpen} onOpenChange={setPowerOpen} />

      <div className={s.main}>
        <div className={s.topbar}>
          {!atRoot && (
            <Tooltip content="Voltar para a tela anterior" relationship="label">
              <Button
                size="small"
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
              appearance="secondary"
              size="small"
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
          <Clock />
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
