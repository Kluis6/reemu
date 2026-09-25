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
  CounterBadge,
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
import { useTranslation } from "react-i18next";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { AnimatedBackground } from "../components/AnimatedBackground";
import { ButtonHints } from "../components/ButtonHints";
import { Clock } from "../components/Clock";
import { GamepadStatus } from "../components/GamepadStatus";
import { NotificationBell } from "../components/NotificationBell";
import { PowerMenuDialog } from "../components/PowerMenuDialog";
import { ProfileAvatar } from "../components/ProfileAvatar";
import { RouteTransition } from "../components/RouteTransition";
import { UpdateDialog } from "../components/UpdateDialog";
import { useFullscreen } from "../hooks/useFullscreen";
import { useUpdateCheck } from "../hooks/useUpdateCheck";
import { getProfile, listPendingMatches, quitApp } from "../lib/tauri";
import { useSearchStore } from "../stores/useSearchStore";
import { useShellStyles } from "../styles/xbox";

const useLocalStyles = makeStyles({
  // ícones da sidebar com o border-radius padrão do botão do Fluent
  railRadius: { borderRadius: tokens.borderRadiusMedium, position: "relative" },
  // contador de pendências de metadata no ícone de Configurações
  railBadge: {
    position: "absolute",
    top: "2px",
    right: "2px",
    pointerEvents: "none",
  },
  // Voltar/Fullscreen: mesmo tom de fundo da sidebar (`rail`,
  // `colorNeutralBackground2`) no fundo E na borda — a borda fica sempre da
  // mesma cor do fundo (em repouso e no hover), então nunca aparece como uma
  // linha separada. `border` (não `borderColor`: o Griffel não aceita esse
  // shorthand isolado, só `shorthands.borderColor()` — ver
  // griffel.js.org/react/guides/limitations).
  navBtn: {
    backgroundColor: `${tokens.colorNeutralBackground2} !important`,
    border: `1px solid ${tokens.colorNeutralBackground2} !important`,
    ":hover": {
      backgroundColor: `${tokens.colorNeutralBackground2Hover} !important`,
      border: `1px solid ${tokens.colorNeutralBackground2Hover} !important`,
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
    label: "nav.home",
  },
  {
    to: "/library",
    end: true,
    icon: <LibraryRegular />,
    activeIcon: <LibraryFilled />,
    label: "nav.library",
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
    label: "nav.settings",
  },
] as const;

// Menu do avatar. Conquistas/rede ainda não têm tela — ficam desabilitados.
// "Sair" fecha o app.
const PROFILE_EXTRA = [
  { icon: <TrophyRegular />, label: "nav.achievements" },
  { icon: <PeopleRegular />, label: "nav.network" },
] as const;

export function AppShell() {
  const { t } = useTranslation();
  const s = useShellStyles();
  const l = useLocalStyles();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const { on: fullscreen, toggle: toggleFullscreen } = useFullscreen();
  const atRoot = pathname === "/";
  const atBrowse = pathname === "/" || pathname === "/library";
  const [powerOpen, setPowerOpen] = useState(false);
  useUpdateCheck();
  // Correspondências de metadata esperando revisão (Configurações ›
  // Metadata). Mesma query da tela de revisão: resolver lá atualiza aqui.
  const pending = useQuery({
    queryKey: ["pending-matches"],
    queryFn: listPendingMatches,
    retry: false,
    staleTime: 30_000,
  });
  const pendingCount = pending.data?.length ?? 0;

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
        { glyph: "A", label: t("hints.select") },
        { glyph: "Y", label: t("hints.search") },
        { glyph: "MENU", label: t("hints.options") },
      ] as const)
    : ([
        { glyph: "A", label: t("hints.select") },
        { glyph: "B", label: t("hints.back") },
      ] as const);
  return (
    <div className={s.app}>
      <AnimatedBackground showWallpaper={atRoot} />
      <nav className={s.rail}>
        <Menu positioning={{ position: "below", align: "start", offset: 12 }}>
          <MenuTrigger disableButtonEnhancement>
            <button
              className={s.railBrand}
              aria-label={t("shell.profile")}
              type="button"
            >
              <ProfileAvatar
                profile={
                  profile.data ?? { name: "Jogador", avatar: "preset:1" }
                }
                size={32}
                className={s.railAvatarSize}
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
                  {t(m.label)}
                </MenuItem>
              ))}
              <MenuDivider />
              <Menu>
                <MenuTrigger disableButtonEnhancement>
                  <MenuItem disabled>{t("shell.status")}</MenuItem>
                </MenuTrigger>
                <MenuPopover className={l.menuPopover}>
                  <MenuList className={l.menuBody}>
                    <MenuItem>{t("shell.online")}</MenuItem>
                    <MenuItem>{t("shell.away")}</MenuItem>
                    <MenuItem>{t("shell.playing")}</MenuItem>
                  </MenuList>
                </MenuPopover>
              </Menu>

              <MenuItem onClick={() => void quitApp()}>
                {t("shell.quit")}
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
            title={t(it.label)}
            aria-label={
              it.to === "/settings" && pendingCount > 0
                ? t("shell.pendingMetadata", {
                    label: t(it.label),
                    count: pendingCount,
                  })
                : t(it.label)
            }
          >
            {({ isActive }) => (
              <>
                {isActive ? it.activeIcon : it.icon}
                {it.to === "/settings" && pendingCount > 0 && (
                  <CounterBadge
                    className={l.railBadge}
                    count={pendingCount}
                    overflowCount={99}
                    size="small"
                    color="brand"
                  />
                )}
              </>
            )}
          </NavLink>
        ))}
        <div className={s.railSpacer} />

        <NotificationBell
          className={mergeClasses(s.railItem, s.railQuit, l.railRadius)}
        />
        <div className={s.railSep} />
        <Tooltip content={t("shell.shutdown")} relationship="label">
          <Button
            className={mergeClasses(s.railItem, s.railQuit, l.railRadius)}
            onClick={() => setPowerOpen(true)}
            aria-label={t("shell.shutdown")}
            appearance="subtle"
            icon={<PowerRegular />}
          />
        </Tooltip>
      </nav>

      <PowerMenuDialog open={powerOpen} onOpenChange={setPowerOpen} />
      <UpdateDialog />

      <div className={s.main}>
        <div className={s.topbar}>
          {!atRoot && (
            <Tooltip content={t("shell.backTooltip")} relationship="label">
              <Button
                appearance="secondary"
                className={mergeClasses(l.navBtn, s.navIconBtn)}
                icon={<ChevronLeftRegular />}
                aria-label={t("hints.back")}
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
            placeholder={t("shell.searchPlaceholder")}
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
              className={mergeClasses(l.navBtn, s.navIconBtn)}
              aria-label={
                fullscreen ? t("shell.exitFullscreen") : t("shell.fullscreen")
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
          <GamepadStatus />
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
