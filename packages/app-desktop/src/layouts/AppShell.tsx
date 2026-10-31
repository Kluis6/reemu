import {
  ChevronLeftRegular,
  FullScreenMaximizeRegular,
  FullScreenMinimizeRegular,
  HomeFilled,
  HomeRegular,
  LibraryFilled,
  LibraryRegular,
  PowerRegular,
  SettingsFilled,
  SettingsRegular,
} from "@fluentui/react-icons";
import {
  Button,
  CounterBadge,
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
import { useWallpaperStore } from "../stores/useWallpaperStore";
import { Clock } from "../components/Clock";
import { GamepadStatus } from "../components/GamepadStatus";
import { NetworkStatus } from "../components/NetworkStatus";
import { NotificationBell } from "../components/NotificationBell";
import { PowerMenuDialog } from "../components/PowerMenuDialog";
import { ProfileDrawer } from "../components/ProfileDrawer";
import { ProfileAvatar } from "../components/ProfileAvatar";
import { RouteTransition } from "../components/RouteTransition";
import { UpdateDialog } from "../components/UpdateDialog";
import { ShaderFailureDialog } from "../components/ShaderFailureDialog";
import { useCoreUpdates } from "../hooks/useCoreUpdates";
import { useFocusMemory } from "../hooks/useFocusMemory";
import { useNavHints } from "../hooks/useNavHints";
import { useShaderFailures } from "../hooks/useShaderFailures";
import { useFullscreen } from "../hooks/useFullscreen";
import { useUpdateCheck } from "../hooks/useUpdateCheck";
import { getProfile, listPendingMatches } from "../lib/tauri";
import { useSearchStore } from "../stores/useSearchStore";
import { useShellStyles } from "../styles/xbox";
import { TOOLTIP_POSITION } from "../lib/tooltip";

const useLocalStyles = makeStyles({
  // ícones da sidebar com o border-radius padrão do botão do Fluent
  // Cantos de 6 epx, como o item ativo da rail do app Xbox.
  railRadius: { borderRadius: tokens.borderRadiusLarge, position: "relative" },
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


export function AppShell() {
  const { t } = useTranslation();
  const s = useShellStyles();
  const l = useLocalStyles();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const { on: fullscreen, toggle: toggleFullscreen } = useFullscreen();
  const atRoot = pathname === "/";
  const wallpaperScope = useWallpaperStore((st) => st.scope);
  const [powerOpen, setPowerOpen] = useState(false);
  const [profileOpen, setProfileOpen] = useState(false);
  useUpdateCheck();
  useShaderFailures();
  useCoreUpdates();
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
  useFocusMemory(scrollRef);

  const hints = useNavHints(() => scrollRef.current);
  return (
    <div className={mergeClasses("reemu-app", s.app)}>
      {/* Papel de parede: só na tela inicial ou em todas as telas (escolha em
          Aparência › Papel de parede); nunca na página do jogo, que tem a
          arte do jogo no hero. */}
      <AnimatedBackground
        showWallpaper={wallpaperScope === "all" ? !pathname.startsWith("/rom/") : atRoot}
      />
      <nav className={s.rail} data-nav-rail>
        {/* Avatar: abre o perfil num drawer à esquerda (era um menu suspenso;
            pedido do usuário, 2026-10-29). `Button` do Fluent sem fundo
            próprio (o do `railItem`) — estados e foco padrão. */}
        <Button
          appearance="transparent"
          className={mergeClasses(s.railItem, l.railRadius, s.railBrand)}
          aria-label={t("shell.profile")}
          aria-haspopup="dialog"
          onClick={() => setProfileOpen(true)}
        >
          <ProfileAvatar
            profile={profile.data ?? { name: "Jogador", avatar: "preset:1" }}
            size={32}
            className={s.railAvatarSize}
            ring={SHOW_AVATAR_RING}
            badge={SHOW_AVATAR_BADGE ? "available" : undefined}
          />
        </Button>

        {RAIL.map((it) => (
          <Tooltip key={it.to} positioning={TOOLTIP_POSITION} content={t(it.label)} relationship="label">
          <NavLink
            to={it.to}
            end={it.end}
            className={mergeClasses(s.railItem, l.railRadius)}
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
          </Tooltip>
        ))}
        <div className={s.railSpacer} />

        <NotificationBell
          className={mergeClasses(s.railItem, s.railQuit, l.railRadius)}
        />
        <div className={s.railSep} />
        <Tooltip positioning={TOOLTIP_POSITION} content={t("shell.shutdown")} relationship="label">
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
      <ProfileDrawer
        open={profileOpen}
        onOpenChange={setProfileOpen}
        profile={profile.data ?? { name: "Jogador", avatar: "preset:1", bio: null }}
      />
      <UpdateDialog />
      <ShaderFailureDialog />

      <div className={s.main}>
        <div className={s.topbar} data-nav-topbar>
          {!atRoot && (
            <Tooltip positioning={TOOLTIP_POSITION} content={t("shell.backTooltip")} relationship="label">
              <Button
                appearance="secondary"
                className={mergeClasses("reemu-nav-back", l.navBtn, s.navIconBtn, s.navIconLg)}
                icon={<ChevronLeftRegular />}
                aria-label={t("hints.back")}
                onClick={() => navigate(-1)}
              />
            </Tooltip>
          )}
          <SearchBox
            ref={searchRef}
            // `reemu-no-ring`: busca sem o anel de foco (index.css)
            className={mergeClasses("reemu-no-ring", s.search, search.keyboard && !search.keyboardTarget && s.searchTyping)}
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
          <Tooltip positioning={TOOLTIP_POSITION}
            content={
              fullscreen
                ? t("shell.exitFullscreenTip")
                : t("shell.fullscreenTip")
            }
            relationship="label"
          >
            <Button
              appearance="secondary"
              className={mergeClasses(l.navBtn, s.navIconBtn, s.navIconLg)}
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
          <NetworkStatus />
          <Clock />
        </div>
        <div className={s.scroll} ref={scrollRef} data-nav-content={pathname.startsWith("/rom/") ? "bleed" : ""}>
          <RouteTransition
            routeKey={pathname.startsWith("/settings") ? "/settings" : pathname}
          >
            <Outlet />
          </RouteTransition>
        </div>
      </div>

      <ButtonHints hints={search.keyboard ? [] : hints} />
    </div>
  );
}
