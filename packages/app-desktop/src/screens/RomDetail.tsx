import {
  Body1,
  Text,
  Badge,
  Button,
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
  Select,
  Tab,
  TabList,
  Tooltip,
} from "@fluentui/react-components";
import {
  ArrowResetRegular,
  DeleteFilled,
  DeleteRegular,
  DismissRegular,
  EditRegular,
  HeartFilled,
  HeartRegular,
  InfoRegular,
  PlayRegular,
} from "@fluentui/react-icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { CoreOptions } from "../components/CoreOptions";
import { SearchArt } from "../components/EmptyArt";
import { EmptyState, LoadingState } from "../components/EmptyState";
import { SaveStateThumb } from "../components/SaveStateThumb";
import { ShaderLibrary } from "../components/ShaderLibrary";
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
import { errorToast, sysToast } from "../lib/toast";
import {
  deleteSaveState,
  getRomMetadata,
  getRomShader,
  getShaderInfo,
  listBiosStatus,
  listInstalledCores,
  listRoms,
  listSaveStates,
  pickSlangp,
  listSystemCores,
  removeRom,
  setRomFavorite,
  setRomMetadata,
  setShader,
  getRomPlayTime,
  type ShaderScope,
} from "../lib/tauri";
import { formatPlayTime } from "../lib/playTime";
import { useDetailStyles } from "../styles/xbox";
import { useToastStore } from "../stores/useToastStore";
import { useTranslation } from "react-i18next";

export function RomDetail() {
  const { t, i18n } = useTranslation();
  const s = useDetailStyles();
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
  // Escopo onde as edições de shader são gravadas: este jogo / a plataforma /
  // todos os jogos.
  const [shaderScope, setShaderScope] = useState<ShaderScope>("rom");
  // backend deriva de `romId` se vazio; preenche quando `romShader` carrega.
  const shaderSystemId = romShader.data?.systemId ?? "";
  const shaderPick = useMutation({
    mutationFn: (name: string) =>
      setShader(name, shaderScope, romId, shaderSystemId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["rom-shader", romId] });
      qc.invalidateQueries({ queryKey: ["shader-info"] });
    },
    onError: (e) => push(errorToast(e, "changeGameShader")),
  });
  // O que está atribuído EXATAMENTE no escopo selecionado ("" = herda).
  const shaderAtScope =
    (shaderScope === "rom"
      ? romShader.data?.atRom
      : shaderScope === "system"
        ? romShader.data?.atSystem
        : romShader.data?.atDefault) ?? "";
  const currentGameShader = shaderAtScope
    ? (shaderAtScope.split(/[/\\]/).pop() ?? shaderAtScope)
    : "";

  const rom = roms.data?.find((r) => r.id === romId);
  const ext = rom?.filePath.split(".").pop()?.toLowerCase() ?? "";
  const coreList = [...(cores.data ?? [])].sort((a, b) => {
    const am = a.extensions.includes(ext) ? 0 : 1;
    const bm = b.extensions.includes(ext) ? 0 : 1;
    return am - bm || a.name.localeCompare(b.name);
  });
  const [coreId, setCoreId] = useState("");
  const systemDefaultCore = rom ? (sysCores.data?.[rom.systemId] ?? "") : "";
  const chosenCore =
    coreId || systemDefaultCore || coreList[0]?.coreId || "";
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const [editName, setEditName] = useState("");
  const [editSystem, setEditSystem] = useState("");
  const [infoOpen, setInfoOpen] = useState(false);
  // Campos da gaveta formatados pelo que cada provedor documenta
  // (lib/metadataFormat.ts).
  const releaseText = formatReleaseDate(meta.data?.releaseDate);
  const genres = splitGenres(meta.data?.genre);
  const descParas = descriptionParagraphs(meta.data?.description);
  const sourceText = providerLabel(meta.data?.providerSource);
  const releaseYear = meta.data?.releaseDate?.match(/^\d{4}/)?.[0] ?? null;
  const [cfgTab, setCfgTab] = useState<"core" | "states" | "shader">("core");
  const hasShaderCfg = !!shaderInfo.data?.gpu;
  const hasCoreCfg = !!chosenCore;
  const cfgTabOk = { shader: hasShaderCfg, core: hasCoreCfg, states: true };
  const activeCfgTab = cfgTabOk[cfgTab]
    ? cfgTab
    : hasCoreCfg
      ? "core"
      : hasShaderCfg
        ? "shader"
        : "states";

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

  const play = (loadState?: string) => {
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
        <div className={s.heroBody}>
          <div className={s.heroHeader}>
            {cover && <img className={s.heroIcon} src={cover} alt="" />}
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
              appearance="primary"
              size="large"
              className={s.playBtn}
              disabled={!chosenCore}
              onClick={() => play()}
            >
              {hasQuick ? t("game.continue") : t("game.play")}
            </Button>
            {/* Ações secundárias só com ícone (pedido do usuário); o nome vem
                pelo tooltip e pelo aria-label. */}
            <Tooltip
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
            <Tooltip content={t("game.edit")} relationship="label">
              <Button
                size="large"
                appearance="secondary"
                className={mergeClasses(s.noBorderButton, s.heroActionBtn)}
                icon={<EditRegular />}
                onClick={openEdit}
              />
            </Tooltip>
            <Tooltip content={t("game.info")} relationship="label">
              <Button
                size="large"
                appearance="secondary"
                className={mergeClasses(s.noBorderButton, s.heroActionBtn)}
                icon={<InfoRegular />}
                onClick={() => setInfoOpen(true)}
              />
            </Tooltip>
            <Tooltip
              content={
                confirmRemove ? t("game.removeConfirm") : t("game.remove")
              }
              relationship="label"
            >
              <Button
                size="large"
                appearance={confirmRemove ? "primary" : "secondary"}
                className={mergeClasses(
                  s.noBorderButton,
                  s.heroActionBtn,
                  s.dangerBtn,
                  confirmRemove && s.dangerConfirm,
                )}
                icon={confirmRemove ? <DeleteFilled /> : <DeleteRegular />}
                disabled={remove.isPending}
                onClick={() => {
                  if (confirmRemove) remove.mutate();
                  else {
                    setConfirmRemove(true);
                    window.setTimeout(() => setConfirmRemove(false), 3000);
                  }
                }}
              />
            </Tooltip>
          </div>
          {coreList.length === 0 && (
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
              <Field
                label={t("common.platform")}
                hint={t("game.platformHint")}
              >
                <Select
                  value={editSystem}
                  onChange={(_, d) => setEditSystem(d.value)}
                >
                  {!knownPlatforms().some(([id]) => id === editSystem) && (
                    <option value={editSystem}>
                      {platformLabel(editSystem)}
                    </option>
                  )}
                  {knownPlatforms().map(([id, label]) => (
                    <option key={id} value={id}>
                      {label}
                    </option>
                  ))}
                </Select>
              </Field>
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
            action={
              <Button
                appearance="subtle"
                aria-label={t("common.close")}
                icon={<DismissRegular />}
                onClick={() => setInfoOpen(false)}
              />
            }
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
                {sourceText && (
                  <>
                    <dt className={s.infoLabel}>{t("game.source")}</dt>
                    <dd className={s.infoValue}>{sourceText}</dd>
                  </>
                )}
              </dl>
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

      <section className={mergeClasses(s.section, s.tabsOverlap)}>
        <TabList
          selectedValue={activeCfgTab}
          onTabSelect={(_, d) =>
            setCfgTab(d.value as "core" | "states" | "shader")
          }
        >
          {hasCoreCfg && <Tab value="core">{t("game.tabs.core")}</Tab>}
          <Tab value="states">{t("game.tabs.states")}</Tab>
          {hasShaderCfg && <Tab value="shader">{t("game.tabs.shader")}</Tab>}
        </TabList>
        <div className={s.panel}>
          {activeCfgTab === "shader" && shaderInfo.data?.gpu && (
            <>
              <div className={s.field}>
                <Caption1>{t("game.shader.applyTo")}</Caption1>
                <div style={{ overflowX: "auto" }}>
                  <TabList
                    size="small"
                    selectedValue={shaderScope}
                    onTabSelect={(_, d) =>
                      setShaderScope(d.value as ShaderScope)
                    }
                  >
                    <Tab value="rom">{t("game.shader.scopeGame")}</Tab>
                    <Tab value="system">
                      {rom ? platformLabel(rom.systemId) : t("common.platform")}
                    </Tab>
                  </TabList>
                </div>
                <Caption1 className={s.hint}>
                  {romShader.data?.resolvedScope === "rom"
                    ? t("game.shader.activeRom")
                    : romShader.data?.resolvedScope === "system"
                      ? t("game.shader.activeSystem")
                      : romShader.data?.resolvedScope === "default"
                        ? t("game.shader.activeDefault")
                        : t("game.shader.activeNone")}
                </Caption1>
              </div>
              <Select
                size="small"
                value={currentGameShader}
                disabled={shaderPick.isPending}
                onChange={(_, d) => shaderPick.mutate(d.value)}
              >
                <option value="">
                  {shaderScope === "default" ? t("common.none") : t("game.shader.inherit")}
                </option>
                {shaderInfo.data.available.map((n) => (
                  <option key={n} value={n}>
                    {n}
                  </option>
                ))}
                {shaderInfo.data.curated.map((c) => (
                  <option key={c.id} value={c.id} disabled={!c.available}>
                    {c.label}
                    {c.available ? "" : t("game.shader.needsPack")}
                  </option>
                ))}
                {shaderAtScope &&
                  !shaderInfo.data.available.includes(shaderAtScope) &&
                  !shaderInfo.data.curated.some((c) => c.id === shaderAtScope) && (
                    <option value={shaderAtScope}>{currentGameShader}</option>
                  )}
              </Select>
              <ShaderLibrary
                onPick={(p) => shaderPick.mutate(p)}
                activePath={shaderInfo.data.active}
                busy={shaderPick.isPending}
              />
              <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                <Button
                  size="small"
                  appearance="subtle"
                  disabled={shaderPick.isPending}
                  onClick={async () => {
                    const p = await pickSlangp();
                    if (p) shaderPick.mutate(p);
                  }}
                >
                  {t("game.shader.loadFile")}
                </Button>
                {shaderAtScope && shaderScope !== "default" && (
                  <Button
                    size="small"
                    appearance="subtle"
                    icon={<ArrowResetRegular />}
                    disabled={shaderPick.isPending}
                    onClick={() => shaderPick.mutate("")}
                  >
                    {t("game.shader.inheritRemove")}
                  </Button>
                )}
              </div>
              {shaderAtScope && (
                <ShaderParams
                  scope={shaderScope}
                  romId={romId}
                  systemId={shaderSystemId}
                  reloadKey={`${shaderScope}:${currentGameShader}`}
                />
              )}
            </>
          )}
          {activeCfgTab === "core" && chosenCore && (
            <>
              <Field label={t("game.core")} className={s.coreField}>
                <Select
                  value={chosenCore}
                  disabled={coreList.length === 0}
                  onChange={(_, d) => setCoreId(d.value)}
                >
                  {coreList.length === 0 && (
                    <option value="">{t("game.noneInstalled")}</option>
                  )}
                  {coreList.map((c) => (
                    <option key={c.coreId} value={c.coreId}>
                      {c.name}
                      {c.extensions.includes(ext) ? " ✓" : ""}
                    </option>
                  ))}
                </Select>
              </Field>
              <CoreOptions coreId={chosenCore} romId={romId} />
            </>
          )}
          {activeCfgTab === "states" && (
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
                    <Caption1 className={s.hint}>
                      {new Date(st.createdAt * 1000).toLocaleString(i18n.language)}
                      {st.playTimeAtSave != null &&
                        t("game.playedFor", { time: formatPlayTime(st.playTimeAtSave) })}
                    </Caption1>
                  </div>
                  <Button
                    size="small"
                    icon={<PlayRegular />}
                    disabled={!chosenCore}
                    onClick={() => play(st.id)}
                  >
                    {t("game.playFromHere")}
                  </Button>
                  <Button
                    size="small"
                    appearance="subtle"
                    disabled={del.isPending}
                    onClick={() => del.mutate(st.id)}
                  >
                    {t("common.delete")}
                  </Button>
                </div>
              ))}
            </>
          )}
        </div>
      </section>

      {meta.data?.description && (
        <Text as="p" block className={s.desc}>
          {meta.data.description}
        </Text>
      )}

    </div>
  );
}
