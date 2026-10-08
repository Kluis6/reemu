import {
  Body1,
  Subtitle1,
  Title3,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";
import { ProfileAvatar } from "../../components/ProfileAvatar";
import { SETTINGS_SECTIONS } from "../../layouts/settingsSections";
import { getProfile } from "../../lib/tauri";

// Folga pro anel de foco (3 + 4 de afastamento) não ser cortado pela borda
// da grade, que rola se a tela for baixa demais.
const RING = 8;

const cardIn = {
  from: { opacity: 0, transform: "translateY(12px) scale(0.98)" },
  to: { opacity: 1, transform: "none" },
};

const useStyles = makeStyles({
  // Bento: 11 categorias, Perfil ocupando 2 colunas = 12 células, que fecham
  // 3×4, 4×3 ou 6×2 sem buraco. As linhas dividem a altura útil (`1fr`),
  // então tudo cabe sem rolar. As colunas seguem a PROPORÇÃO da janela, não a
  // largura: com o zoom da interface (`uiScale.ts`) a tela lógica continua
  // 16:9 e só encolhe (Maior = 1051×591), e aí 4 colunas × 3 linhas é o que
  // cabe; 3 colunas só em janela mais quadrada (4:3, 5:4), que tem altura
  // pra 4 linhas. Abaixo de 72 px por card a grade rola em vez de espremer.
  grid: {
    flexGrow: 1,
    minHeight: 0,
    display: "grid",
    gridTemplateColumns: "repeat(4, minmax(0, 1fr))",
    gridAutoRows: "minmax(72px, 1fr)",
    gap: tokens.spacingHorizontalM,
    overflowY: "auto",
    scrollbarWidth: "none",
    padding: `${RING}px`,
    margin: `-${RING}px`,
    "@media (max-aspect-ratio: 3/2)": {
      gridTemplateColumns: "repeat(3, minmax(0, 1fr))",
    },
    "@media (max-width: 640px)": {
      gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    },
  },
  // `<a>`: ganha o anel de foco global do app (controle/teclado). Card
  // clicável do Fluent 2: a superfície inteira navega, hover/pressionado
  // mudam o fundo e a elevação sobe no hover.
  // Ícone e texto juntos no ALTO do card, embaixo fica livre: a grade desce
  // até a área segura de baixo (como o card das categorias), e com controle
  // conectado a barra de dicas (canto inferior direito) cobre o pé do card
  // de Sistema — com o nome lá embaixo, ela escondia o texto.
  card: {
    display: "flex",
    flexDirection: "column",
    justifyContent: "flex-start",
    gap: tokens.spacingVerticalL,
    minWidth: 0,
    padding: tokens.spacingHorizontalXXL,
    boxSizing: "border-box",
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground2,
    boxShadow: tokens.shadow4,
    color: tokens.colorNeutralForeground1,
    textDecorationLine: "none",
    overflow: "hidden",
    transitionProperty: "background-color, box-shadow",
    transitionDuration: tokens.durationFaster,
    transitionTimingFunction: tokens.curveEasyEase,
    ":hover": {
      backgroundColor: tokens.colorNeutralBackground2Hover,
      boxShadow: tokens.shadow8,
    },
    ":active": { backgroundColor: tokens.colorNeutralBackground2Pressed },
    // entrada em cascata (o atraso vem do índice, no `style`)
    animationName: cardIn,
    animationDuration: "380ms",
    animationTimingFunction: tokens.curveDecelerateMid,
    // `backwards`: ver o comentário em RouteTransition (ClearType no WebView2)
    animationFillMode: "backwards",
    "@media (prefers-reduced-motion: reduce)": { animationName: "none" },
    // Tela baixa (interface Grande/Maior): o card fica em linha, ícone ao
    // lado do texto, pra caber na altura de uma linha da grade.
    "@media (max-height: 640px)": {
      flexDirection: "row",
      alignItems: "center",
      justifyContent: "flex-start",
      gap: tokens.spacingHorizontalL,
      padding: `${tokens.spacingVerticalM} ${tokens.spacingHorizontalL}`,
    },
  },
  profile: {
    gridColumn: "span 2",
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "flex-start",
    gap: tokens.spacingHorizontalXL,
  },
  icon: {
    fontSize: "40px",
    "@media (max-height: 640px)": { fontSize: "32px" },
    flexShrink: 0,
    color: tokens.colorBrandForeground1,
  },
  text: {
    display: "flex",
    flexDirection: "column",
    gap: "2px",
    minWidth: 0,
  },
  desc: {
    color: tokens.colorNeutralForeground3,
    display: "-webkit-box",
    WebkitLineClamp: 2,
    WebkitBoxOrient: "vertical",
    overflow: "hidden",
  },
  // Tela baixa (card em linha): título menor e podendo quebrar linha, senão
  // nomes longos ("Gerenciar biblioteca") saíam cortados.
  name: {
    "@media (max-height: 640px)": {
      fontSize: tokens.fontSizeBase400,
      lineHeight: tokens.lineHeightBase400,
      whiteSpace: "normal",
    },
  },
  ellipsis: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
});

/**
 * Entrada das Configurações: um card por categoria em grade bento, que ocupa a
 * área útil da página sem rolar. O card de Perfil é o maior e mostra o avatar
 * e o nome; os outros, ícone, nome e o que tem dentro.
 */
export function SettingsHome() {
  const { t } = useTranslation();
  const s = useStyles();
  const profile = useQuery({
    queryKey: ["profile"],
    queryFn: getProfile,
    retry: false,
  });

  return (
    <nav className={s.grid} aria-label={t("settings.title")}>
      {SETTINGS_SECTIONS.map(({ key, id, Icon }, i) => {
        const delay = { animationDelay: `${i * 30}ms` };
        if (id === "profile") {
          const name = profile.data?.name ?? "";
          return (
            <Link
              key={key}
              to={key}
              className={mergeClasses(s.card, s.profile)}
              style={delay}
            >
              {profile.data ? (
                <ProfileAvatar profile={profile.data} size={96} />
              ) : (
                <Icon className={s.icon} />
              )}
              <span className={s.text}>
                <Title3 className={s.ellipsis}>
                  {name || t("settings.tabs.profile")}
                </Title3>
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
            <Icon className={s.icon} />
            <span className={s.text}>
              <Subtitle1 className={mergeClasses(s.ellipsis, s.name)}>
                {t(`settings.tabs.${id}`)}
              </Subtitle1>
              <Body1 className={s.desc}>{t(`settings.desc.${id}`)}</Body1>
            </span>
          </Link>
        );
      })}
    </nav>
  );
}
