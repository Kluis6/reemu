import {
  Body1,
  Subtitle2,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";
import { ProfileAvatar } from "../../components/ProfileAvatar";
import { useEffectiveProfile } from "../../hooks/useEffectiveProfile";
import { SETTINGS_SECTIONS } from "../../layouts/settingsSections";

// Folga pro anel de foco (3 de vão + 3 de traço) e o brilho de fora (~12)
// não serem cortados pela borda da grade, que rola se a tela for baixa
// demais.
const RING = 20;

const cardIn = {
  from: { opacity: 0, transform: "translateY(12px) scale(0.98)" },
  to: { opacity: 1, transform: "none" },
};

const useStyles = makeStyles({
  // Cards em 2 colunas, como as Configurações do Xbox (print do usuário,
  // 2026-11-14): 12 categorias = 6 linhas. Altura entre 72 e 104 px; se a
  // tela for baixa demais, a grade rola em vez de espremer. Janela estreita:
  // uma coluna.
  grid: {
    flexGrow: 1,
    minHeight: 0,
    display: "grid",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    gridAutoRows: "minmax(72px, 104px)",
    alignContent: "start",
    gap: tokens.spacingHorizontalS,
    overflowY: "auto",
    scrollbarWidth: "none",
    padding: `${RING}px`,
    margin: `-${RING}px`,
    "@media (max-width: 640px)": {
      gridTemplateColumns: "minmax(0, 1fr)",
    },
  },
  // `<a>`: ganha o anel de foco global do app (controle/teclado). Card
  // clicável do Fluent 2 em layout horizontal: ícone à esquerda, nome e
  // descrição ao lado, a superfície inteira navega; hover e pressionado
  // mudam o fundo.
  card: {
    display: "flex",
    flexDirection: "row",
    alignItems: "center",
    gap: tokens.spacingHorizontalL,
    minWidth: 0,
    padding: `${tokens.spacingVerticalM} ${tokens.spacingHorizontalXL}`,
    boxSizing: "border-box",
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground2,
    // Sem sombra (pedido do usuário, como os cards do modo XBOX): o card se
    // destaca da página só pela cor; hover/pressionado mudam o fundo.
    color: tokens.colorNeutralForeground1,
    textDecorationLine: "none",
    // visível: o brilho do foco (`::before`, index.css) fica fora do card;
    // o texto corta com reticências nele mesmo
    position: "relative",
    overflow: "visible",
    transitionProperty: "background-color",
    transitionDuration: tokens.durationFaster,
    transitionTimingFunction: tokens.curveEasyEase,
    // No foco só o anel (index.css): o fundo não muda — no tema claro o
    // `Background2Hover` escurecia o card focado (print do usuário,
    // 2026-10-29). O hover continua mudando o fundo.
    ":hover": {
      backgroundColor: tokens.colorNeutralBackground2Hover,
    },
    ":active": {
      backgroundColor: tokens.colorNeutralBackground2Pressed,
    },
    // entrada em cascata (o atraso vem do índice, no `style`)
    animationName: cardIn,
    animationDuration: tokens.durationSlower,
    animationTimingFunction: tokens.curveDecelerateMid,
    // `backwards`: ver o comentário em RouteTransition (ClearType no WebView2)
    animationFillMode: "backwards",
    "@media (prefers-reduced-motion: reduce)": { animationName: "none" },
  },
  // Ícone de traço, na cor do texto, como no Xbox; mesma largura em todos
  // os cards para os nomes ficarem alinhados na coluna.
  icon: {
    flexShrink: 0,
    width: "32px",
    display: "flex",
    justifyContent: "center",
    fontSize: "32px",
    lineHeight: 0,
    color: tokens.colorNeutralForeground1,
  },
  text: {
    display: "flex",
    flexDirection: "column",
    gap: "2px",
    minWidth: 0,
  },
  // Descrição numa linha só: com controle conectado a barra de dicas
  // (canto inferior direito) cobria a 2ª linha do card de Sistema.
  desc: {
    color: tokens.colorNeutralForeground3,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  ellipsis: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  // Subcategoria (Aparência, Decorações): os mesmos cards do hub, numa
  // fileira só (não estica até o fim da página).
  subGrid: {
    flexGrow: 0,
  },
});

export interface SettingsHubItem {
  to: string;
  icon: ReactElement;
  title: string;
  description: string;
}

/**
 * Cards de uma subcategoria das Configurações (Aparência › Temas / Papel de
 * parede, Decorações › Shaders / Molduras) com a MESMA cara dos cards do
 * hub, direto na página (sem o card geral atrás — ele só aparece dentro da
 * opção escolhida; pedido do usuário).
 */
export function SettingsHubCards({ items, label }: { items: SettingsHubItem[]; label: string }) {
  const s = useStyles();
  return (
    <nav className={mergeClasses(s.grid, s.subGrid)} aria-label={label}>
      {items.map((it, i) => (
        <Link key={it.to} to={it.to} className={s.card} style={{ animationDelay: `${i * 30}ms` }}>
          <span className={s.icon}>{it.icon}</span>
          <span className={s.text}>
            <Subtitle2 className={s.ellipsis}>{it.title}</Subtitle2>
            <Body1 className={s.desc}>{it.description}</Body1>
          </span>
        </Link>
      ))}
    </nav>
  );
}

/**
 * Entrada das Configurações: um card por categoria em 2 colunas, como as
 * Configurações do Xbox. O card de Perfil mostra o avatar e o nome no lugar
 * do ícone e do nome da categoria; os outros, ícone, nome e o que tem dentro.
 */
export function SettingsHome() {
  const { t } = useTranslation();
  const s = useStyles();
  const profile = useEffectiveProfile();

  return (
    <nav className={s.grid} aria-label={t("settings.title")}>
      {SETTINGS_SECTIONS.map(({ key, id, Icon }, i) => {
        const delay = { animationDelay: `${i * 30}ms` };
        if (id === "profile") {
          const name = profile?.name ?? "";
          return (
            <Link key={key} to={key} className={s.card} style={delay}>
              <span className={s.icon}>
                {profile ? (
                  <ProfileAvatar profile={profile} size={32} tier={profile.tierColors} />
                ) : (
                  <Icon />
                )}
              </span>
              <span className={s.text}>
                <Subtitle2 className={s.ellipsis}>{name || t("settings.tabs.profile")}</Subtitle2>
                <Body1 className={s.desc}>
                  {name
                    ? `${t("settings.tabs.profile")} · ${t("settings.desc.profile")}`
                    : t("settings.desc.profile")}
                </Body1>
              </span>
            </Link>
          );
        }
        return (
          <Link key={key} to={key} className={s.card} style={delay}>
            <span className={s.icon}>
              <Icon />
            </span>
            <span className={s.text}>
              <Subtitle2 className={s.ellipsis}>{t(`settings.tabs.${id}`)}</Subtitle2>
              <Body1 className={s.desc}>{t(`settings.desc.${id}`)}</Body1>
            </span>
          </Link>
        );
      })}
    </nav>
  );
}
