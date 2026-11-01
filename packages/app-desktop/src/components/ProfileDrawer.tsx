import {
  Button,
  Caption1,
  DrawerBody,
  OverlayDrawer,
  Title3,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import {
  ChevronRightRegular,
} from "@fluentui/react-icons";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import symbol from "../assets/alvanista-symbol.svg";
import { ALVANISTA_CACHED_KEY } from "../hooks/useEffectiveProfile";
import { alvanistaCachedStatus, quitApp, type Profile } from "../lib/tauri";
import { BadgeIcon } from "../screens/profile/AlvanistaCardBlock";
import { tierAccent } from "../lib/tierRing";
import { pickText, readableInk } from "../screens/profile/profileData";
import { floatingDrawer } from "../styles/xbox";
import { ProfileAvatar } from "./ProfileAvatar";

// tamanho da escala do `Avatar` do Fluent (20…128); fora dela ele cai no padrão
const AVATAR = 120;

const useStyles = makeStyles({
  // Flutuante à esquerda, como o menu de pausa e o guia do Xbox.
  drawer: {
    ["--fui-Drawer--size" as string]: "min(400px, calc(100vw - 48px))",
    ...floatingDrawer("left"),
    backgroundColor: tokens.colorNeutralBackground2,
    color: tokens.colorNeutralForeground1,
  },
  body: {
    display: "flex",
    flexDirection: "column",
    padding: 0,
  },
  // Como o guia de perfil do Xbox: a capa ocupa todo o topo, atrás da
  // identidade. Sem capa, as cores do tema (o destaque descendo para a cor de
  // marca).
  banner: {
    position: "relative",
    flexShrink: 0,
    backgroundImage: `linear-gradient(160deg, var(--reemuBrandAccent) 0%, ${tokens.colorBrandBackground} 100%)`,
    backgroundSize: "cover",
    backgroundPosition: "center",
  },
  // véu para o texto se separar de qualquer imagem
  bannerShade: {
    position: "absolute",
    inset: 0,
    backgroundImage: `linear-gradient(180deg, transparent 0%, ${tokens.colorNeutralBackground2} 75%)`,
  },
  // linha de cima: o nível à esquerda e a marca da conta à direita
  top: {
    position: "relative",
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    minHeight: "24px",
    padding: "16px 22px 0",
  },
  symbol: { width: "24px", height: "24px", marginLeft: "auto" },
  identity: {
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    rowGap: "4px",
    padding: "8px 24px 16px",
    position: "relative",
    textAlign: "center",
  },
  avatar: {
    borderRadius: tokens.borderRadiusCircular,
    boxShadow: `0 0 0 4px ${tokens.colorNeutralBackground2}`,
  },
  name: { margin: 0, overflowWrap: "anywhere" },
  bio: { color: tokens.colorNeutralForeground3, overflowWrap: "anywhere" },
  nameRow: {
    // o respiro abaixo do avatar (fora dele: com o anel do plano, margem no
    // avatar entraria no invólucro do anel)
    marginTop: "8px",
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    justifyContent: "center",
    gap: "8px",
  },
  chip: {
    padding: "1px 8px",
    borderRadius: tokens.borderRadiusCircular,
    fontSize: "12px",
    backgroundColor: tokens.colorNeutralBackground3,
    color: tokens.colorNeutralForeground1,
  },
  reino: { color: tokens.colorNeutralForeground2 },
  // fileira das badges recentes do Alvanista
  badges: {
    position: "relative",
    display: "flex",
    justifyContent: "center",
    columnGap: "16px",
    margin: "0 16px",
    padding: "12px",
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground3,
    // nenhuma imagem carregou: sem a faixa vazia
    ":empty": { display: "none" },
  },
  badge: { width: "48px", height: "48px", objectFit: "contain" },
  // Lista só de texto, sem caixa em repouso, como no Xbox.
  list: {
    display: "flex",
    flexDirection: "column",
    rowGap: "4px",
    padding: "12px 10px 22px",
  },
  item: {
    justifyContent: "flex-start",
    width: "100%",
    maxWidth: "none",
    minHeight: "48px",
    fontWeight: tokens.fontWeightRegular,
  },
  // seta à direita do item que abre um submenu (Status), como no Xbox
  chevron: { marginLeft: "auto", fontSize: "16px" },
});

/**
 * Perfil num drawer à esquerda, como o guia de perfil do Xbox: a capa atrás do
 * avatar, do nome e da bio, as badges recentes numa fileira e a lista de ações
 * só de texto. Nível, plano, título e badges vêm do cartão do Alvanista e somem
 * sem ele. Rede e status ainda não têm tela: ficam desabilitados.
 */
export function ProfileDrawer({
  open,
  onOpenChange,
  profile,
  bannerUrl,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** O perfil efetivo (`useEffectiveProfile`): conectado ao Alvanista, o de lá. */
  profile: Pick<Profile, "name" | "avatar" | "bio"> & { avatarUrl?: string | null };
  /** Imagem do banner (a capa do Alvanista). Sem ela, as cores do tema. */
  bannerUrl?: string | null;
}) {
  const { t, i18n } = useTranslation();
  const s = useStyles();
  const navigate = useNavigate();
  const go = (to: string) => {
    onOpenChange(false);
    navigate(to);
  };
  const alva = useQuery({
    queryKey: ALVANISTA_CACHED_KEY,
    queryFn: alvanistaCachedStatus,
    retry: false,
    enabled: open,
  });
  const connected = !!alva.data?.connected;
  const card = connected ? alva.data?.card : null;
  const level = card?.identity?.level;
  const tier = pickText(card?.identity?.tier?.name, i18n.language);
  const reino = pickText(card?.reino_title_text, i18n.language);
  const tierColors = card?.identity?.tier?.colors;
  const tint = tierAccent(tierColors);
  const badges = (card?.badges?.recent ?? []).slice(0, 5);
  const item = (to: string, label: string) => (
    <Button size="large" appearance="subtle" className={s.item} onClick={() => go(to)}>
      {label}
    </Button>
  );

  return (
    <OverlayDrawer
      open={open}
      onOpenChange={(_, d) => onOpenChange(d.open)}
      position="start"
      size="small"
      className={s.drawer}
      aria-label={t("shell.profile")}
    >
      <DrawerBody className={s.body}>
        <div
          className={s.banner}
          style={bannerUrl ? { backgroundImage: `url(${JSON.stringify(bannerUrl)})` } : undefined}
        >
          <div className={s.bannerShade} aria-hidden />
          <div className={s.top}>
            {level != null && <span className={s.chip}>{t("profilePage.chipLevel", { level })}</span>}
            {connected && <img className={s.symbol} src={symbol} alt="" aria-hidden draggable={false} />}
          </div>
          <div className={s.identity}>
            <ProfileAvatar profile={profile} size={AVATAR} className={s.avatar} tier={tierColors} />
            <div className={s.nameRow}>
              <Title3 as="h2" className={s.name}>
                {profile.name}
              </Title3>
              {tier && (
                <span
                  className={s.chip}
                  style={tint ? { backgroundColor: tint, color: readableInk(tint) } : undefined}
                >
                  {tier}
                </span>
              )}
            </div>
            {reino && <Caption1 className={s.reino}>{reino}</Caption1>}
            {profile.bio && <Caption1 className={s.bio}>{profile.bio}</Caption1>}
          </div>
          {badges.length > 0 && (
            <div className={s.badges} data-testid="drawer-badges">
              {badges.map((b, i) => (
                <BadgeIcon key={`${b.name}-${i}`} badge={b} className={s.badge} />
              ))}
            </div>
          )}
        </div>

        <div className={s.list}>
          {item("/perfil", t("shell.myProfile"))}
          {item("/conquistas", t("nav.achievements"))}
          <Button size="large" appearance="subtle" className={s.item} disabled>
            {t("nav.network")}
          </Button>
          <Button size="large" appearance="subtle" className={s.item} disabled>
            {t("shell.status")}
            <ChevronRightRegular className={s.chevron} aria-hidden />
          </Button>
          <Button size="large" appearance="subtle" className={s.item} onClick={() => void quitApp()}>
            {t("shell.quit")}
          </Button>
        </div>
      </DrawerBody>
    </OverlayDrawer>
  );
}
