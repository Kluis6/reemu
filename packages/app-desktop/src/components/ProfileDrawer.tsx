import {
  Button,
  Caption1,
  DrawerBody,
  OverlayDrawer,
  Title3,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import {
  ArrowExitRegular,
  ChevronRightRegular,
  PeopleRegular,
  PersonRegular,
  TrophyRegular,
} from "@fluentui/react-icons";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { quitApp, type Profile } from "../lib/tauri";
import { useModalButtonStyles } from "../styles/buttons";
import { floatingDrawer } from "../styles/xbox";
import { ProfileAvatar } from "./ProfileAvatar";

const BANNER_H = 148;
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
  // Banner do perfil: a imagem da rede social quando houver; sem ela, as
  // cores do tema (o destaque descendo pra cor de marca) — pedido do
  // usuário, 2026-10-29.
  banner: {
    position: "relative",
    height: `${BANNER_H}px`,
    flexShrink: 0,
    backgroundImage: `linear-gradient(160deg, var(--reemuBrandAccent) 0%, ${tokens.colorBrandBackground} 100%)`,
    backgroundSize: "cover",
    backgroundPosition: "center",
  },
  // véu embaixo do banner pra o avatar se separar de qualquer imagem
  bannerShade: {
    position: "absolute",
    inset: 0,
    backgroundImage: `linear-gradient(180deg, transparent 40%, ${tokens.colorNeutralBackground2} 100%)`,
  },
  identity: {
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    rowGap: "4px",
    // o avatar sobe metade sobre o banner
    marginTop: `-${AVATAR / 2}px`,
    paddingLeft: "24px",
    paddingRight: "24px",
    paddingBottom: "20px",
    position: "relative",
    textAlign: "center",
  },
  avatar: {
    borderRadius: tokens.borderRadiusCircular,
    boxShadow: `0 0 0 4px ${tokens.colorNeutralBackground2}`,
    marginBottom: "8px",
  },
  name: { margin: 0, overflowWrap: "anywhere" },
  bio: { color: tokens.colorNeutralForeground3, overflowWrap: "anywhere" },
  list: {
    display: "flex",
    flexDirection: "column",
    rowGap: "8px",
    padding: "16px 22px 22px",
  },
  item: {
    justifyContent: "flex-start",
    width: "100%",
    maxWidth: "none",
    columnGap: "12px",
  },
  // seta à direita do item que abre um submenu (Status), como no Xbox
  chevron: { marginLeft: "auto", fontSize: "16px" },
});

/**
 * Perfil num drawer à esquerda, no lugar do menu suspenso do avatar do rail
 * (pedido do usuário, 2026-10-29, print do guia de perfil do Xbox): banner,
 * avatar grande, nome e bio no topo; as ações embaixo. Conquistas, rede e
 * status ainda não têm tela — desabilitados, como no menu de antes.
 */
export function ProfileDrawer({
  open,
  onOpenChange,
  profile,
  bannerUrl,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  profile: Pick<Profile, "name" | "avatar" | "bio">;
  /** Imagem do banner (rede social, futuramente). Sem ela, as cores do tema. */
  bannerUrl?: string | null;
}) {
  const { t } = useTranslation();
  const s = useStyles();
  const btn = useModalButtonStyles();
  const navigate = useNavigate();
  const go = (to: string) => {
    onOpenChange(false);
    navigate(to);
  };
  const item = mergeClasses(s.item, btn.secondary);

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
          aria-hidden
        >
          <div className={s.bannerShade} />
        </div>
        <div className={s.identity}>
          <ProfileAvatar profile={profile} size={AVATAR} className={s.avatar} />
          <Title3 as="h2" className={s.name}>
            {profile.name}
          </Title3>
          {profile.bio && <Caption1 className={s.bio}>{profile.bio}</Caption1>}
        </div>

        <div className={s.list}>
          <Button
            size="large"
            className={item}
            icon={<PersonRegular />}
            onClick={() => go("/settings/perfil")}
          >
            {t("shell.myProfile")}
          </Button>
          <Button size="large" className={item} icon={<TrophyRegular />} disabled>
            {t("nav.achievements")}
          </Button>
          <Button size="large" className={item} icon={<PeopleRegular />} disabled>
            {t("nav.network")}
          </Button>
          <Button size="large" className={item} disabled>
            {t("shell.status")}
            <ChevronRightRegular className={s.chevron} aria-hidden />
          </Button>
          <Button
            size="large"
            className={item}
            icon={<ArrowExitRegular />}
            onClick={() => void quitApp()}
          >
            {t("shell.quit")}
          </Button>
        </div>
      </DrawerBody>
    </OverlayDrawer>
  );
}
