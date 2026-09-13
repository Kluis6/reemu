import {
  Body1,
  Text,
  Button,
  Caption1,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Field,
  Input,
  Select,
  Tab,
  TabList,
  Tooltip,
} from "@fluentui/react-components";
import {
  ArrowLeftRegular,
  ArrowResetRegular,
  DeleteRegular,
  EditRegular,
  PlayRegular,
  StarFilled,
  StarRegular,
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
import { DIALOG_FADE_ONLY } from "../lib/motion";
import { knownPlatforms, platformLabel } from "../lib/platform";
import { sysToast } from "../lib/toast";
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
  type ShaderScope,
} from "../lib/tauri";
import { useDetailStyles } from "../styles/xbox";
import { useToastStore } from "../stores/useToastStore";

export function RomDetail() {
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
    onError: (e) => push(sysToast(`Falha: ${e}`, "Error")),
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
  const [cfgTab, setCfgTab] = useState<"shader" | "core" | "states">("shader");
  const hasShaderCfg = !!shaderInfo.data?.gpu;
  const hasCoreCfg = !!chosenCore;
  const cfgTabOk = { shader: hasShaderCfg, core: hasCoreCfg, states: true };
  const activeCfgTab = cfgTabOk[cfgTab]
    ? cfgTab
    : hasShaderCfg
      ? "shader"
      : hasCoreCfg
        ? "core"
        : "states";

  const del = useMutation({
    mutationFn: (id: string) => deleteSaveState(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["save-states", romId] }),
    onError: (e) => push(sysToast(`Falha ao apagar: ${e}`, "Error")),
  });
  const remove = useMutation({
    mutationFn: () => removeRom(romId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["roms"] });
      push(
        sysToast(`"${rom?.title ?? "ROM"}" removida da biblioteca.`, "Success"),
      );
      navigate("/library");
    },
    onError: (e) => push(sysToast(`Falha ao remover: ${e}`, "Error")),
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
      push(sysToast("Dados da ROM atualizados.", "Success"));
    },
    onError: (e) => push(sysToast(`Falha ao salvar: ${e}`, "Error")),
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
      push(sysToast(`Falha ao favoritar: ${e}`, "Error"));
    },
    onSettled: () => qc.invalidateQueries({ queryKey: ["roms"] }),
  });

  if (roms.isLoading) return <LoadingState />;
  if (!rom)
    return (
      <EmptyState
        art={<SearchArt />}
        title="ROM não encontrada"
        action={
          <Button appearance="primary" onClick={() => navigate("/library")}>
            Voltar pra biblioteca
          </Button>
        }
      >
        Ela pode ter sido removida da biblioteca.
      </EmptyState>
    );

  const cover = meta.data?.coverUrl ?? rom.boxart;
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
          `Falta o BIOS obrigatório de ${platformLabel(rom.systemId)} (${missingRequiredBios.filename}) — o jogo pode não rodar. Veja Configurações › BIOS.`,
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
      <Button
        className={s.back}
        appearance="subtle"
        size="small"
        icon={<ArrowLeftRegular />}
        onClick={() => navigate(-1)}
      >
        Voltar
      </Button>

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
          <Text as="h1" className={s.title}>
            {title}
          </Text>
          <div className={s.badges}>
            <span className={s.badge}>{platformLabel(rom.systemId)}</span>
            {meta.data?.releaseDate && (
              <span className={s.badge}>{meta.data.releaseDate}</span>
            )}
            {meta.data?.genre && (
              <span className={s.badge}>{meta.data.genre}</span>
            )}
          </div>
          {meta.data?.description && (
            <Text as="p" className={s.desc}>
              {meta.data.description}
            </Text>
          )}
          <div className={s.path}>{rom.filePath}</div>
        </div>
      </div>

      <div className={s.actions}>
        <Field label="Core">
          <Select
            value={chosenCore}
            disabled={coreList.length === 0}
            onChange={(_, d) => setCoreId(d.value)}
          >
            {coreList.length === 0 && (
              <option value="">nenhum instalado</option>
            )}
            {coreList.map((c) => (
              <option key={c.coreId} value={c.coreId}>
                {c.name}
                {c.extensions.includes(ext) ? " ✓" : ""}
              </option>
            ))}
          </Select>
        </Field>
        <Button
          appearance="primary"
          size="large"
          icon={<PlayRegular />}
          disabled={!chosenCore}
          onClick={() => play()}
        >
          {hasQuick ? "Continuar" : "Jogar"}
        </Button>
        <Tooltip
          content={
            rom.isFavorite
              ? "Remover dos favoritos"
              : "Adicionar aos favoritos"
          }
          relationship="label"
        >
          <Button
            size="large"
            appearance={rom.isFavorite ? "outline" : "subtle"}
            icon={rom.isFavorite ? <StarFilled /> : <StarRegular />}
            aria-pressed={rom.isFavorite}
            onClick={() => fav.mutate(!rom.isFavorite)}
          >
            {rom.isFavorite ? "Favorito" : "Favoritar"}
          </Button>
        </Tooltip>
        <Tooltip content="Editar nome e plataforma" relationship="label">
          <Button
            size="large"
            appearance="subtle"
            icon={<EditRegular />}
            onClick={openEdit}
          >
            Editar
          </Button>
        </Tooltip>
      </div>
      {coreList.length === 0 && (
        <Caption1>
          Instale um core em Configurações → Cores pra poder jogar.
        </Caption1>
      )}

      <Dialog
        open={editOpen}
        onOpenChange={(_, d) => setEditOpen(d.open)}
        surfaceMotion={DIALOG_FADE_ONLY}
      >
        <DialogSurface>
          <DialogBody>
            <DialogTitle>Editar ROM</DialogTitle>
            <DialogContent
              style={{ display: "flex", flexDirection: "column", gap: 16 }}
            >
              <Field label="Nome">
                <Input
                  value={editName}
                  onChange={(_, d) => setEditName(d.value)}
                  placeholder={rom.title}
                />
              </Field>
              <Field
                label="Plataforma"
                hint="Corrige ROMs que o scan não identificou (ficam em 'Disco')."
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
                Cancelar
              </Button>
              <Button
                appearance="primary"
                disabled={editMeta.isPending}
                onClick={() => editMeta.mutate()}
              >
                Salvar
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>

      <section className={s.section}>
        <TabList
          selectedValue={activeCfgTab}
          onTabSelect={(_, d) =>
            setCfgTab(d.value as "shader" | "core" | "states")
          }
        >
          {hasShaderCfg && <Tab value="shader">Shader</Tab>}
          {hasCoreCfg && <Tab value="core">Emulador</Tab>}
          <Tab value="states">Save states</Tab>
        </TabList>
        <div className={s.panel}>
          {activeCfgTab === "shader" && shaderInfo.data?.gpu && (
            <>
              <div className={s.field}>
                <Caption1>Aplicar a</Caption1>
                <div style={{ overflowX: "auto" }}>
                  <TabList
                    size="small"
                    selectedValue={shaderScope}
                    onTabSelect={(_, d) =>
                      setShaderScope(d.value as ShaderScope)
                    }
                  >
                    <Tab value="rom">Jogo</Tab>
                    <Tab value="system">
                      {rom ? platformLabel(rom.systemId) : "Plataforma"}
                    </Tab>
                    <Tab value="default">Todos</Tab>
                  </TabList>
                </div>
                <Caption1 className={s.hint}>
                  {romShader.data?.resolvedScope === "rom"
                    ? "Ativo: definido neste jogo."
                    : romShader.data?.resolvedScope === "system"
                      ? "Ativo: herdado da plataforma."
                      : romShader.data?.resolvedScope === "default"
                        ? "Ativo: herdado de todos os jogos."
                        : "Ativo: nenhum (shader 'plain')."}
                </Caption1>
              </div>
              <Select
                value={currentGameShader}
                disabled={shaderPick.isPending}
                onChange={(_, d) => shaderPick.mutate(d.value)}
              >
                <option value="">
                  {shaderScope === "default" ? "Nenhum" : "Herdar"}
                </option>
                {shaderInfo.data.available.map((n) => (
                  <option key={n} value={n}>
                    {n}
                  </option>
                ))}
                {shaderInfo.data.curated.map((c) => (
                  <option key={c.id} value={c.id} disabled={!c.available}>
                    {c.label}
                    {c.available ? '' : ' (baixe o pacote de shaders)'}
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
                  Carregar .slangp avulso…
                </Button>
                {shaderAtScope && shaderScope !== "default" && (
                  <Button
                    size="small"
                    appearance="subtle"
                    icon={<ArrowResetRegular />}
                    disabled={shaderPick.isPending}
                    onClick={() => shaderPick.mutate("")}
                  >
                    Herdar (remover deste escopo)
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
            <CoreOptions coreId={chosenCore} romId={romId} />
          )}
          {activeCfgTab === "states" && (
            <>
              {states.data?.length === 0 && (
                <Caption1>Nenhum save state pra esta ROM.</Caption1>
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
                          ? `Slot ${st.slot}`
                          : "Auto"}
                    </Body1>
                    <Caption1 className={s.hint}>
                      {new Date(st.createdAt * 1000).toLocaleString()}
                    </Caption1>
                  </div>
                  <Button
                    size="small"
                    icon={<PlayRegular />}
                    disabled={!chosenCore}
                    onClick={() => play(st.id)}
                  >
                    Jogar daqui
                  </Button>
                  <Button
                    size="small"
                    appearance="subtle"
                    disabled={del.isPending}
                    onClick={() => del.mutate(st.id)}
                  >
                    Apagar
                  </Button>
                </div>
              ))}
            </>
          )}
        </div>
      </section>

      <section className={s.section}>
        <Button
          appearance="subtle"
          icon={<DeleteRegular />}
          disabled={remove.isPending}
          onClick={() => {
            if (confirmRemove) remove.mutate();
            else {
              setConfirmRemove(true);
              window.setTimeout(() => setConfirmRemove(false), 3000);
            }
          }}
        >
          {confirmRemove
            ? "Clique de novo para confirmar"
            : "Remover da biblioteca"}
        </Button>
        <Caption1 className={s.hint}>
          Remove só da lista — o arquivo em disco fica e um novo scan
          readiciona.
        </Caption1>
      </section>
    </div>
  );
}
