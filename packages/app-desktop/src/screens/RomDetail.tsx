import {
  Body1,
  Text,
  Badge,
  Button,
  Spinner,
  Caption1,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  DrawerBody,
  DrawerHeader,
  DrawerHeaderTitle,
  Field,
  Input,
  mergeClasses,
  MessageBar,
  MessageBarActions,
  MessageBarBody,
  OverlayDrawer,
  Tooltip,
  tokens,
} from "@fluentui/react-components";
import {
  ArrowSyncRegular,
  ClockRegular,
  DeleteFilled,
  DeleteRegular,
  EditRegular,
  HeartFilled,
  HeartRegular,
  InfoRegular,
  PlayRegular,
  TrophyRegular,
  WarningRegular,
} from "@fluentui/react-icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { FullscreenDialog } from "../components/FullscreenDialog";
import { ConflictDialog } from "../components/sync/ConflictDialog";
import { SaveLine } from "../components/sync/SaveLine";
import { GamePresets } from "../components/GamePresets";
import { GameImageOptions } from "../components/GameImageOptions";
import { SelectBlock } from "../components/SettingBlock";
import { SearchArt } from "../components/EmptyArt";
import { EmptyState, LoadingState } from "../components/EmptyState";
import { CheatsPanel } from "../components/CheatsPanel";
import { SaveStateThumb } from "../components/SaveStateThumb";
import { RemoteStateList } from "../components/sync/RemoteStateList";
import { ShaderPicker } from "../components/ShaderPicker";
import { ShaderParams } from "../components/ShaderParams";
import {
  descriptionParagraphs,
  formatDateTime,
  formatReleaseDate,
  providerLabel,
  splitGenres,
  splitPath,
} from "../lib/metadataFormat";
import { DIALOG_FADE_ONLY } from "../lib/motion";
import { knownPlatforms, platformLabel } from "../lib/platform";
import { errorToast, sysToast, shaderIssueToast } from "../lib/toast";
import { errorText } from "../lib/errors";
import {
  deleteSaveState,
  getRomMetadata,
  rescrapeRom,
  getRomShader,
  getShaderInfo,
  getShaderParams,
  listBiosStatus,
  listInstalledCores,
  listRoms,
  listSaveStates,
  listSystemCores,
  removeRom,
  setRomFavorite,
  setRomMetadata,
  setShader,
  getRomPlayTime,
  syncConflicts,
  syncSaveStatus,
  type SyncConflict,
} from "../lib/tauri";
import { formatPlayTime } from "../lib/playTime";
import { useModalButtonStyles } from "../styles/buttons";
import { useDetailStyles } from "../styles/xbox";
import { useToastStore } from "../stores/useToastStore";
import { useTranslation } from "react-i18next";
import { rankCores } from "../lib/coreChoice";
import { TOOLTIP_POSITION } from "../lib/tooltip";
import { CloseButton } from "../components/CloseButton";

export function RomDetail() {
  const { t, i18n } = useTranslation();
  const s = useDetailStyles();
  const btn = useModalButtonStyles();
  const navigate = useNavigate();
  const qc = useQueryClient();
  const push = useToastStore((st) => st.push);
  const { romId = "" } = useParams();

  const roms = useQuery({
    queryKey: ["roms"],
    queryFn: listRoms,
    retry: false,
  });
  const cores = useQuery({
    queryKey: ["installed-cores"],
    queryFn: listInstalledCores,
    retry: false,
  });
  const sysCores = useQuery({
    queryKey: ["system-cores"],
    queryFn: listSystemCores,
    retry: false,
  });
  const playTime = useQuery({
    queryKey: ["play-time", romId],
    queryFn: () => getRomPlayTime(romId),
    retry: false,
    // Sempre atual ao abrir a página: o tempo muda a cada partida e a
    // volta do jogo não invalida esta query (o cache padrão é de 5 min).
    staleTime: 0,
    refetchOnMount: "always",
  });
  const states = useQuery({
    queryKey: ["save-states", romId],
    queryFn: () => listSaveStates(romId),
    retry: false,
  });
  const meta = useQuery({
    queryKey: ["rom-metadata", romId],
    queryFn: () => getRomMetadata(romId),
    retry: false,
  });
  // "Buscar de novo" na gaveta de informações: consulta os provedores só
  // pra este jogo e conta o resultado num toast.
  const rescrape = useMutation({
    mutationFn: () => rescrapeRom(romId),
    onSuccess: (r) => {
      qc.invalidateQueries({ queryKey: ["rom-metadata", romId] });
      qc.invalidateQueries({ queryKey: ["roms"] });
      qc.invalidateQueries({ queryKey: ["pending-matches"] });
      const key =
        r === "auto" ? "game.rescrapeAuto" : r === "pending" ? "game.rescrapePending" : "game.rescrapeNone";
      push(sysToast(t(key), r === "auto" ? "Success" : "Info"));
    },
    onError: (e) =>
      push(
        // o erro agora chega como `AppError` (objeto): o texto fica em `message`
        errorText(e).includes("ss_dev_login")
          ? sysToast(t("metadata.errors.ssDevLoginTitle"), "Warning")
          : errorToast(e, "fetchMetadata"),
      ),
  });
  const shaderInfo = useQuery({
    queryKey: ["shader-info"],
    queryFn: getShaderInfo,
    retry: false,
  });
  // Cacheado (Config › BIOS já deixa isso quente); só pra avisar antes de
  // jogar um sistema que precisa de BIOS obrigatório e ele estiver faltando.
  const biosStatus = useQuery({
    queryKey: ["bios-status"],
    queryFn: listBiosStatus,
    retry: false,
  });
  const romShader = useQuery({
    queryKey: ["rom-shader", romId],
    queryFn: () => getRomShader(romId),
    retry: false,
  });
  // A página do jogo só edita o shader deste jogo (pedido do usuário, 2026-10-31).
  // backend deriva de `romId` se vazio; preenche quando `romShader` carrega.
  const shaderSystemId = romShader.data?.systemId ?? "";
  const shaderPick = useMutation({
    mutationFn: (name: string) =>
      setShader(name, "rom", romId, shaderSystemId),
    onSuccess: (issue, name) => {
      qc.invalidateQueries({ queryKey: ["rom-shader", romId] });
      qc.invalidateQueries({ queryKey: ["shader-info"] });
      // preset com problema conhecido na GPU (gpu/known_issues.txt)
      if (issue) {
        const label = name.split(/[/\\]/).pop()?.replace(/\.slangp$/i, "") ?? name;
        push(shaderIssueToast(t(`video.shaderIssue.${issue}`, { name: label })));
      }
    },
    onError: (e) => push(errorToast(e, "changeGameShader")),
  });
  // O que está atribuído exatamente a este jogo ("" = herda).
  const shaderAtScope = romShader.data?.atRom ?? "";
  const currentGameShader = shaderAtScope
    ? (shaderAtScope.split(/[/\\]/).pop() ?? shaderAtScope)
    : "";
  // Parâmetros do shader do jogo — mesma chave do
  // `ShaderParams`, só pra saber se a coluna de ajustes aparece.
  const shaderParamsKey = `rom:${currentGameShader}`;
  const shaderParams = useQuery({
    queryKey: ["shader-params", "rom", romId, shaderSystemId, shaderParamsKey],
    queryFn: getShaderParams,
    enabled: !!shaderAtScope,
    retry: false,
  });

  const rom = roms.data?.find((r) => r.id === romId);
  // Ao abrir a página de um jogo, o foco vai pro "Jogar" (controle e teclado
  // já começam nele). Uma vez por jogo — refetch da lista não rouba o foco.
  const playRef = useRef<HTMLButtonElement>(null);
  // A checagem do save em andamento: outro Jogar nesse intervalo é ignorado.
  const checkingRef = useRef(false);
  const hasRom = !!rom;
  useEffect(() => {
    if (hasRom) playRef.current?.focus({ preventScroll: true });
  }, [romId, hasRom]);
  const ext = rom?.filePath.split(".").pop()?.toLowerCase() ?? "";
  const coreList = rankCores(cores.data ?? [], rom?.systemId, ext);
  const [coreId, setCoreId] = useState("");
  const systemDefaultCore = rom ? (sysCores.data?.[rom.systemId] ?? "") : "";
  const chosenCore =
    coreId || systemDefaultCore || coreList[0]?.coreId || "";
  const [confirmRemove, setConfirmRemove] = useState(false);
  // Dica do "Remover": abre sozinha no 1º clique (a confirmação) e fica
  // aberta enquanto ela vale — com controle ou toque não há hover pra
  // mostrá-la. Fora disso, abre no hover/foco como as outras.
  const [removeTipHover, setRemoveTipHover] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  // capa que não carregou (URL quebrada): cai no card sem imagem
  const [brokenCover, setBrokenCover] = useState<string | null>(null);
  const [editName, setEditName] = useState("");
  const [editSystem, setEditSystem] = useState("");
  const [infoOpen, setInfoOpen] = useState(false);
  // Conflito de save achado ao apertar Jogar: pergunta antes de abrir.
  const [conflictAsk, setConflictAsk] = useState<{ conflict: SyncConflict; loadState?: string } | null>(null);
  // Campos da gaveta formatados pelo que cada provedor documenta
  // (lib/metadataFormat.ts).
  const releaseText = formatReleaseDate(meta.data?.releaseDate);
  const genres = splitGenres(meta.data?.genre);
  const modes = splitGenres(meta.data?.modes);
  const descParas = descriptionParagraphs(meta.data?.description);
  const sourceText = providerLabel(meta.data?.providerSource);
  const releaseYear = meta.data?.releaseDate?.match(/^\d{4}/)?.[0] ?? null;
  // Dialog de tela cheia aberto; o conteúdo de Emulador fica na própria página.
  const [panel, setPanel] = useState<"states" | "shader" | "cheats" | "achievements" | "media" | null>(null);
  const hasShaderCfg = !!shaderInfo.data?.gpu;
  // Galeria: só as mídias que o provedor trouxe.
  const shots = [
    { key: "screenshot", url: meta.data?.screenshotUrl },
    { key: "titleScreen", url: meta.data?.titleScreenUrl },
    { key: "fanart", url: meta.data?.fanartUrl },
  ].filter((m): m is { key: "screenshot" | "titleScreen" | "fanart"; url: string } => !!m.url);
  const hasMedia = shots.length > 0 || !!meta.data?.videoUrl || !!meta.data?.logoUrl;
  const panels = [
    { key: "states", show: true },
    { key: "shader", show: hasShaderCfg },
    // cheats por código (doc 17): a lista é do jogo, vale ao abrir
    { key: "cheats", show: true },
    { key: "achievements", show: true },
    { key: "media", show: hasMedia },
  ] as const;

  const del = useMutation({
    mutationFn: (id: string) => deleteSaveState(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["save-states", romId] }),
    onError: (e) => push(errorToast(e, "deleteSaveState")),
  });
  const remove = useMutation({
    mutationFn: () => removeRom(romId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["roms"] });
      push(
        sysToast(t("game.removed", { title: rom?.title ?? "ROM" }), "Success"),
      );
      navigate("/library");
    },
    onError: (e) => push(errorToast(e, "removeGame")),
  });
  const editMeta = useMutation({
    mutationFn: () =>
      setRomMetadata(
        romId,
        editName === (rom?.title ?? "") ? undefined : editName,
        editSystem === rom?.systemId ? undefined : editSystem,
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["roms"] });
      qc.invalidateQueries({ queryKey: ["rom-metadata", romId] });
      setEditOpen(false);
      push(sysToast(t("game.updated"), "Success"));
    },
    onError: (e) => push(errorToast(e, "saveChanges")),
  });
  const openEdit = () => {
    setEditName(rom?.title ?? "");
    setEditSystem(rom?.systemId ?? "");
    setEditOpen(true);
  };
  const fav = useMutation({
    mutationFn: (on: boolean) => setRomFavorite(romId, on),
    onMutate: async (on) => {
      await qc.cancelQueries({ queryKey: ["roms"] });
      const prev = qc.getQueryData(["roms"]);
      qc.setQueryData<{ id: string; isFavorite: boolean }[]>(["roms"], (old) =>
        (old ?? []).map((r) => (r.id === romId ? { ...r, isFavorite: on } : r)),
      );
      return { prev };
    },
    onError: (e, _on, ctx) => {
      if (ctx?.prev) qc.setQueryData(["roms"], ctx.prev);
      push(errorToast(e, "favoriteGame"));
    },
    onSettled: () => qc.invalidateQueries({ queryKey: ["roms"] }),
  });

  if (roms.isLoading) return <LoadingState />;
  if (!rom)
    return (
      <EmptyState
        art={<SearchArt />}
        title={t("game.notFound")}
        action={
          <Button appearance="primary" onClick={() => navigate("/library")}>
            {t("game.backToLibrary")}
          </Button>
        }
      >
        {t("game.notFoundHint")}
      </EmptyState>
    );

  const cover = meta.data?.coverUrl ?? rom.boxart;
  const fileParts = splitPath(rom.filePath);
  const title = meta.data?.title ?? rom.title;
  const hasQuick = states.data?.some((st) => st.slot === 0) ?? false;

  // Antes de abrir: um conflito de game save é perguntado (com "Decidir depois"
  // abrindo mesmo assim). A consulta é só disco e SQLite; qualquer falha (sem
  // conta, offline, sync desligado) abre o jogo normalmente: ela nunca bloqueia.
  // Sem sync, revogado ou com os game saves desligados o comando devolve `none`.
  const play = (loadState?: string) => {
    if (!chosenCore || checkingRef.current) return;
    checkingRef.current = true;
    void (async () => {
      try {
        const line = await syncSaveStatus(rom.id);
        if (line.state === "conflict" && line.key !== null) {
          const conflict = (await syncConflicts()).find((c) => c.key === line.key);
          if (conflict) {
            checkingRef.current = false;
            setConflictAsk({ conflict, loadState });
            return;
          }
        }
      } catch {
        // segue para o jogo
      } finally {
        checkingRef.current = false;
      }
      launch(loadState);
    })();
  };

  const launch = (loadState?: string) => {
    if (!chosenCore) return;
    const missingRequiredBios = (biosStatus.data ?? []).find(
      (b) => b.systemId === rom.systemId && b.required && !b.present,
    );
    if (missingRequiredBios) {
      push(
        sysToast(
          t("game.missingBios", { platform: platformLabel(rom.systemId), file: missingRequiredBios.filename }),
          "Warning",
        ),
      );
    }
    const q = new URLSearchParams({ core: chosenCore });
    if (loadState) q.set("loadState", loadState);
    navigate(`/play/${rom.id}?${q}`, {
      state: {
        coreId: chosenCore,
        romPath: rom.filePath,
        title,
        boxart: cover,
        system: platformLabel(rom.systemId),
      },
    });
  };

  return (
    <div className={s.root} data-loading={meta.isPending ? "true" : "false"}>
      <div className={s.hero}>
        <div className={s.heroBackdrop} aria-hidden>
          {cover && (
            <img
              className={s.heroArt}
              src={cover}
              alt=""
              onError={(e) => {
                e.currentTarget.style.display = "none";
              }}
            />
          )}
          <div className={s.heroScrim} />
        </div>
        <div className={s.heroBody}>
          <div className={s.heroHeader}>
            {cover && cover !== brokenCover ? (
              <img
                className={s.heroIcon}
                src={cover}
                alt=""
                onError={() => setBrokenCover(cover)}
              />
            ) : (
              // Sem capa: o mesmo card da galeria sem imagem (superfície de
              // card do tema, contorno fino, nome e plataforma no meio) —
              // pedido do usuário; antes o quadrado sumia.
              <span className={mergeClasses(s.heroIcon, s.heroNoArt)} aria-hidden>
                <span className={s.heroNoArtTitle}>{title}</span>
                <span className={s.heroNoArtSub}>{platformLabel(rom.systemId)}</span>
              </span>
            )}
            <div className={s.heroTitleCol}>
              <span className={s.platform}>{platformLabel(rom.systemId)}</span>
              <Text as="h1" className={s.title}>
                {title}
              </Text>
              {(meta.data?.releaseDate || meta.data?.genre) && (
                <div className={s.badges}>
                  {releaseYear && <span className={s.badge}>{releaseYear}</span>}
                  {genres[0] && <span className={s.badge}>{genres[0]}</span>}
                </div>
              )}
            </div>
          </div>

          <div className={s.actions}>
            <Button
              ref={playRef}
              appearance="primary"
              size="large"
              className={s.playBtn}
              // `disabledFocusable` (não `disabled`): sem core o botão segue
              // focável — recebe o foco e o leitor de tela diz que está
              // indisponível, como o Fluent recomenda pra ação principal.
              disabledFocusable={!chosenCore}
              onClick={() => play()}
            >
              {hasQuick ? t("game.continue") : t("game.play")}
            </Button>
            {/* Ações secundárias só com ícone (pedido do usuário); o nome vem
                pelo tooltip e pelo aria-label. */}
            <Tooltip positioning={TOOLTIP_POSITION}
              content={rom.isFavorite ? t("game.favoriteRemove") : t("game.favoriteAdd")}
              relationship="label"
            >
              <Button
                size="large"
                appearance="secondary"
                className={mergeClasses(s.noBorderButton, s.heroActionBtn)}
                icon={
                  rom.isFavorite ? (
                    <HeartFilled className={s.favIconOn} />
                  ) : (
                    <HeartRegular />
                  )
                }
                aria-pressed={rom.isFavorite}
                onClick={() => fav.mutate(!rom.isFavorite)}
              />
            </Tooltip>
            <Tooltip positioning={TOOLTIP_POSITION} content={t("game.edit")} relationship="label">
              <Button
                size="large"
                appearance="secondary"
                className={mergeClasses(s.noBorderButton, s.heroActionBtn)}
                icon={<EditRegular />}
                onClick={openEdit}
              />
            </Tooltip>
            <Tooltip positioning={TOOLTIP_POSITION} content={t("game.info")} relationship="label">
              <Button
                size="large"
                appearance="secondary"
                className={mergeClasses(s.noBorderButton, s.heroActionBtn)}
                icon={<InfoRegular />}
                onClick={() => setInfoOpen(true)}
              />
            </Tooltip>
            <Tooltip positioning={TOOLTIP_POSITION}
              content={
                confirmRemove ? t("game.removeConfirm") : t("game.remove")
              }
              relationship="label"
              // `visible`/`onVisibleChange`: visibilidade controlada (props
              // do <Tooltip> do Fluent v9)
              visible={confirmRemove || removeTipHover}
              onVisibleChange={(_, d) => setRemoveTipHover(d.visible)}
            >
              <Button
                size="large"
                appearance={confirmRemove ? "primary" : "secondary"}
                className={mergeClasses(
                  s.noBorderButton,
                  s.heroActionBtn,
                  confirmRemove && s.dangerConfirm,
                )}
                icon={confirmRemove ? <DeleteFilled /> : <DeleteRegular />}
                disabled={remove.isPending}
                onClick={() => {
                  if (confirmRemove) remove.mutate();
                  else {
                    setConfirmRemove(true);
                    // tempo pra ler a dica e apertar de novo
                    window.setTimeout(() => setConfirmRemove(false), 5000);
                  }
                }}
              />
            </Tooltip>
          </div>
          {/* Tempo de jogo abaixo das ações, como na página de jogo do app
              Xbox ("Tempo jogado: 20 minutos"). */}
          {playTime.data !== undefined && (
            <div className={s.heroStats}>
              <ClockRegular className={s.heroStatsIcon} />
              <span>
                {playTime.data > 0
                  ? t("game.timePlayed", { time: formatPlayTime(playTime.data) })
                  : t("game.neverPlayed")}
              </span>
            </div>
          )}
          <SaveLine romId={romId} className={s.heroStats} iconClassName={s.heroStatsIcon} />
          {/* Só depois de carregar a lista de cores: antes ela vem vazia e o
              aviso piscava ao abrir a página. */}
          {cores.isSuccess && coreList.length === 0 && (
            <MessageBar intent="warning" className={s.noCoreBar}>
              <MessageBarBody>
                {t("game.noCore", { platform: platformLabel(rom.systemId) })}
              </MessageBarBody>
              <MessageBarActions>
                <Button appearance="primary" onClick={() => navigate("/settings/cores")}>
                  {t("common.openCores")}
                </Button>
              </MessageBarActions>
            </MessageBar>
          )}
        </div>
      </div>

      <Dialog
        open={editOpen}
        onOpenChange={(_, d) => setEditOpen(d.open)}
        surfaceMotion={DIALOG_FADE_ONLY}
      >
        <DialogSurface>
          <DialogBody>
            <DialogTitle>{t("game.editTitle")}</DialogTitle>
            <DialogContent
              style={{ display: "flex", flexDirection: "column", gap: 16 }}
            >
              <Field label={t("common.name")}>
                <Input
                  value={editName}
                  onChange={(_, d) => setEditName(d.value)}
                  placeholder={rom.title}
                />
              </Field>
              {/* seletor no padrão dos blocos (o mesmo da página do jogo) */}
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <SelectBlock
                  raised
                  label={t("common.platform")}
                  value={editSystem}
                  options={[
                    ...(knownPlatforms().some(([id]) => id === editSystem)
                      ? []
                      : [{ value: editSystem, label: platformLabel(editSystem) }]),
                    ...knownPlatforms().map(([id, label]) => ({ value: id, label })),
                  ]}
                  onChange={setEditSystem}
                />
                <Caption1 style={{ color: tokens.colorNeutralForeground3 }}>
                  {t("game.platformHint")}
                </Caption1>
              </div>
            </DialogContent>
            <DialogActions>
              <Button
                appearance="secondary"
                onClick={() => setEditOpen(false)}
              >
                {t("common.cancel")}
              </Button>
              <Button
                appearance="primary"
                disabled={editMeta.isPending}
                onClick={() => editMeta.mutate()}
              >
                {t("common.save")}
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>

      <OverlayDrawer
        position="end"
        size="medium"
        className={s.infoDrawer}
        open={infoOpen}
        onOpenChange={(_, d) => setInfoOpen(d.open)}
      >
        <DrawerHeader>
          <DrawerHeaderTitle
            action={<CloseButton onClick={() => setInfoOpen(false)} />}
          >
            {t("game.infoTitle")}
          </DrawerHeaderTitle>
        </DrawerHeader>
        <DrawerBody>
          <div className={s.infoBody}>
            {cover && <img className={s.infoCover} src={cover} alt="" />}

            <section className={s.infoSection} aria-labelledby="info-sobre">
              <h3 id="info-sobre" className={s.infoHeading}>
                {t("game.about")}
              </h3>
              <dl className={s.infoList}>
                <dt className={s.infoLabel}>{t("game.title")}</dt>
                <dd className={s.infoValue}>{title}</dd>
                <dt className={s.infoLabel}>{t("common.platform")}</dt>
                <dd className={s.infoValue}>{platformLabel(rom.systemId)}</dd>
                {releaseText && (
                  <>
                    <dt className={s.infoLabel}>{t("game.release")}</dt>
                    <dd className={s.infoValue}>{releaseText}</dd>
                  </>
                )}
                {genres.length > 0 && (
                  <>
                    <dt className={s.infoLabel}>{genres.length > 1 ? t("game.genres") : t("game.genre")}</dt>
                    <dd className={mergeClasses(s.infoValue, s.infoTags)}>
                      {genres.map((g) => (
                        <Badge key={g} appearance="tint" color="brand" shape="rounded" size="large">
                          {g}
                        </Badge>
                      ))}
                    </dd>
                  </>
                )}
                {/* Ficha técnica: cada linha só aparece se o provedor trouxe */}
                {meta.data?.developer && (
                  <>
                    <dt className={s.infoLabel}>{t("game.developer")}</dt>
                    <dd className={s.infoValue}>{meta.data.developer}</dd>
                  </>
                )}
                {meta.data?.publisher && (
                  <>
                    <dt className={s.infoLabel}>{t("game.publisher")}</dt>
                    <dd className={s.infoValue}>{meta.data.publisher}</dd>
                  </>
                )}
                {meta.data?.players && (
                  <>
                    <dt className={s.infoLabel}>{t("game.players")}</dt>
                    <dd className={s.infoValue}>{meta.data.players}</dd>
                  </>
                )}
                {modes.length > 0 && (
                  <>
                    <dt className={s.infoLabel}>{t("game.modes")}</dt>
                    <dd className={mergeClasses(s.infoValue, s.infoTags)}>
                      {modes.map((m) => (
                        <Badge key={m} appearance="tint" color="informative" shape="rounded" size="large">
                          {m}
                        </Badge>
                      ))}
                    </dd>
                  </>
                )}
                {meta.data?.rating != null && (
                  <>
                    <dt className={s.infoLabel}>{t("game.rating")}</dt>
                    <dd className={s.infoValue}>
                      {t("game.ratingValue", {
                        value: (meta.data.rating / 10).toLocaleString(i18n.language, {
                          maximumFractionDigits: 1,
                        }),
                      })}
                    </dd>
                  </>
                )}
                {meta.data?.ageRating && (
                  <>
                    <dt className={s.infoLabel}>{t("game.ageRating")}</dt>
                    <dd className={s.infoValue}>{meta.data.ageRating}</dd>
                  </>
                )}
                {sourceText && (
                  <>
                    <dt className={s.infoLabel}>{t("game.source")}</dt>
                    <dd className={s.infoValue}>{sourceText}</dd>
                  </>
                )}
              </dl>
              <Button
                className={s.infoRescrape}
                appearance="secondary"
                icon={rescrape.isPending ? <Spinner size="tiny" /> : <ArrowSyncRegular />}
                disabled={rescrape.isPending}
                onClick={() => rescrape.mutate()}
              >
                {rescrape.isPending ? t("game.rescraping") : t("game.rescrape")}
              </Button>
            </section>

            {descParas.length > 0 && (
              <section className={s.infoSection} aria-labelledby="info-desc">
                <h3 id="info-desc" className={s.infoHeading}>
                  {t("game.description")}
                </h3>
                {descParas.map((p, i) => (
                  <p key={i} className={s.infoPara}>
                    {p}
                  </p>
                ))}
              </section>
            )}

            <section className={s.infoSection} aria-labelledby="info-lib">
              <h3 id="info-lib" className={s.infoHeading}>
                {t("game.inLibrary")}
              </h3>
              <dl className={s.infoList}>
                <dt className={s.infoLabel}>{t("game.file")}</dt>
                <dd className={s.infoValue}>
                  <span className={s.infoFileName}>{fileParts.name}</span>
                  {fileParts.dir && <span className={s.infoFileDir}>{fileParts.dir}</span>}
                </dd>
                <dt className={s.infoLabel}>{t("game.addedAt")}</dt>
                <dd className={s.infoValue}>{formatDateTime(rom.addedAt)}</dd>
                <dt className={s.infoLabel}>{t("game.lastPlayed")}</dt>
                <dd className={s.infoValue}>
                  {rom.lastPlayedAt ? formatDateTime(rom.lastPlayedAt) : t("game.neverPlayed")}
                </dd>
                <dt className={s.infoLabel}>{t("game.playTime")}</dt>
                <dd className={s.infoValue}>
                  {playTime.data ? formatPlayTime(playTime.data) : t("game.neverPlayed")}
                </dd>
                <dt className={s.infoLabel}>{t("game.favorite")}</dt>
                <dd className={s.infoValue}>{rom.isFavorite ? t("common.yes") : t("common.no")}</dd>
              </dl>
            </section>
          </div>
        </DrawerBody>
      </OverlayDrawer>

      <section className={mergeClasses(s.section, s.tabsOverlap, s.tabsCard)}>
        <div className={s.panel}>
          {chosenCore && (
            <>
              <GameImageOptions
                coreId={chosenCore}
                romId={romId}
                leading={
                  <SelectBlock
                    label={t("game.core")}
                    value={chosenCore}
                    disabled={coreList.length === 0}
                    options={
                      coreList.length === 0
                        ? [{ value: "", label: t("game.noneInstalled") }]
                        : coreList.map((c) => ({
                            value: c.coreId,
                            label: `${c.name}${c.extensions.includes(ext) ? " ✓" : ""}`,
                          }))
                    }
                    onChange={setCoreId}
                  />
                }
              />
              <GamePresets coreId={chosenCore} romId={romId} />
            </>
          )}
        <div className={s.panelButtons}>
          {panels
            .filter((p) => p.show)
            .map((p) => (
              <Button
                key={p.key}
                className={mergeClasses(s.panelButton, btn.block)}
                onClick={() => setPanel(p.key)}
              >
                {t(`game.tabs.${p.key}`)}
              </Button>
            ))}
        </div>
        </div>
      </section>

      <FullscreenDialog
        open={panel !== null}
        onClose={() => setPanel(null)}
        title={panel ? t(`game.tabs.${panel}`) : ""}
      >
        <div className={s.dialogPanel}>
          {panel === "states" && (
            <>
              {states.data?.length === 0 && (
                <Caption1>{t("game.noStates")}</Caption1>
              )}
              {states.data?.map((st) => (
                <div key={st.id} className={s.stateRow} style={{ gap: 12 }}>
                  <SaveStateThumb
                    stateId={st.id}
                    hasThumbnail={st.hasThumbnail}
                  />
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <Body1>
                      {st.slot === 0
                        ? "QuickSave"
                        : st.slot != null
                          ? t("game.slot", { n: st.slot })
                          : t("game.auto")}
                    </Body1>
                    <Caption1 block className={s.hint}>
                      {new Date(st.createdAt * 1000).toLocaleString(i18n.language)}
                      {st.playTimeAtSave != null &&
                        t("game.playedFor", { time: formatPlayTime(st.playTimeAtSave) })}
                    </Caption1>
                    {/* gravado com outra versão do core: avisa antes de carregar */}
                    {st.versionChanged && (
                      <Badge appearance="tint" color="warning" icon={<WarningRegular />}>
                        {t("game.otherVersion")}
                      </Badge>
                    )}
                  </div>
                  <Button
                    icon={<PlayRegular />}
                    disabled={!chosenCore}
                    onClick={() => play(st.id)}
                  >
                    {t("game.playFromHere")}
                  </Button>
                  <Button
                    appearance="subtle"
                    disabled={del.isPending}
                    onClick={() => del.mutate(st.id)}
                  >
                    {t("common.delete")}
                  </Button>
                </div>
              ))}
              {/* states de outras máquinas: depois dos locais, sem segurar a página */}
              <RemoteStateList
                romId={rom.id}
                coreId={chosenCore || null}
                onDownloaded={(saved) => {
                  void qc.invalidateQueries({ queryKey: ["save-states", rom.id] });
                  play(saved.id);
                }}
              />
            </>
          )}
          {panel === "shader" && shaderInfo.data?.gpu && (
            <>
              {/* Mesma escolha das Configurações (abas Shaders do ReEmu /
                  Pacote de shaders, em cards), sem a prévia; "Herdar" no
                  topo tira a escolha deste jogo. Os ajustes do shader
                  escolhido (se ele tiver) ficam na coluna ao lado. */}
              <div className={s.shaderCols}>
                <div className={s.shaderPicker}>
                  <ShaderPicker
                    info={shaderInfo.data}
                    selected={shaderAtScope}
                    busy={shaderPick.isPending}
                    onPick={(n) => shaderPick.mutate(n)}
                    inherit={t("game.shader.inheritRom")}
                    fill
                  />
                </div>
                <div className={s.shaderParams}>
                  {shaderAtScope && (shaderParams.data?.length ?? 0) > 0 ? (
                    <ShaderParams
                      scope="rom"
                      romId={romId}
                      systemId={shaderSystemId}
                      reloadKey={shaderParamsKey}
                    />
                  ) : (
                    <Caption1 className={s.hint}>{t("game.shader.noParams")}</Caption1>
                  )}
                </div>
              </div>
            </>
          )}
          {panel === "cheats" && <CheatsPanel romId={romId} />}
          {/* Conquistas deste jogo: ainda sem dados, como no menu de pausa. */}
          {panel === "achievements" && (
            <EmptyState
              art={<TrophyRegular style={{ color: "var(--reemuBrandAccent)", fontSize: "44px" }} />}
              artAccent
              title={t("game.tabs.achievements")}
            >
              {t("play.achievementsSoon")}
            </EmptyState>
          )}
          {panel === "media" && (
            <div className={s.gallery}>
              {meta.data?.logoUrl && (
                <img className={s.galleryLogo} src={meta.data.logoUrl} alt={t("game.media.logo")} />
              )}
              {meta.data?.videoUrl && (
                <video
                  className={s.galleryVideo}
                  src={meta.data.videoUrl}
                  controls
                  muted
                  preload="metadata"
                  aria-label={t("game.media.video")}
                />
              )}
              <div className={s.galleryGrid}>
                {shots.map((m) => (
                  <figure key={m.key} className={s.galleryItem}>
                    <img className={s.galleryShot} src={m.url} alt={t(`game.media.${m.key}`)} loading="lazy" />
                    <figcaption className={s.galleryCaption}>{t(`game.media.${m.key}`)}</figcaption>
                  </figure>
                ))}
              </div>
            </div>
          )}
        </div>
      </FullscreenDialog>

      {conflictAsk && (
        <ConflictDialog
          conflict={conflictAsk.conflict}
          onDone={(outcome) => {
            const { loadState } = conflictAsk;
            setConflictAsk(null);
            // Escolher um lado ou "Decidir depois": o jogo abre. Fechar pelo X,
            // Esc ou Voltar do controle só desiste de abrir.
            if (outcome !== "dismiss") launch(loadState);
          }}
        />
      )}
    </div>
  );
}
