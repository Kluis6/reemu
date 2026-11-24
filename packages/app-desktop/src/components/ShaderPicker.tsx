import { Button, Spinner, Tab, TabList, Tooltip, makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { ArrowSyncRegular } from '@fluentui/react-icons'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { curatedText, presetDesc, presetTitle } from '../lib/backendText'
import type { ShaderInfo } from '../lib/tauri'
import { TOOLTIP_POSITION } from '../lib/tooltip'
import { useTabStyles } from '../styles/xbox'
import { ShaderCard } from './ShaderCard'
import { ShaderLibrary, useShaderPack } from './ShaderLibrary'

const compactHeader = '@container shader-picker (max-width: 490px)'

const useStyles = makeStyles({
  // Lista de shaders com rolagem própria; a folga (8) não deixa o anel de
  // foco ser cortado pela borda da área que rola.
  list: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
    maxHeight: '360px',
    overflowY: 'auto',
    padding: '8px',
    margin: '-8px',
    // Para no card inteiro, com folga no início e no fim. `proximity`: o `mandatory`
    // puxava de volta a rolagem do foco do controle. A folga (8) cobre o anel de foco.
    scrollSnapType: 'y proximity',
    scrollPaddingTop: '8px',
    scrollPaddingBottom: '8px',
    '& > *': { flexShrink: 0, scrollSnapAlign: 'start' },
    '& > *:last-child': { scrollSnapAlign: 'end' },
  },
  // `fill`: a lista ocupa a altura que o pai (flex em coluna) der
  listFill: { flexGrow: 1, minHeight: 0, maxHeight: 'none', marginBottom: 0 },
  // Abas no estilo do app (42 de altura). Nas Configurações a lista começa
  // na altura da imagem da prévia, cuja barra tem 32 + 24: aqui são 10 a
  // mais de aba e 10 a menos de espaço (8 do `gap` do pai + 6).
  header: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    columnGap: tokens.spacingHorizontalS,
    flexShrink: 0,
    marginBottom: '6px',
    containerName: 'shader-picker',
    containerType: 'inline-size',
  },
  // A primeira aba começa na borda da lista de baixo.
  tabs: {
    marginLeft: '0 !important',
    [compactHeader]: {
      '& .fui-Tab': {
        paddingLeft: tokens.spacingHorizontalXS,
        paddingRight: tokens.spacingHorizontalXS,
      },
    },
  },
  update: {
    flexShrink: 0,
    whiteSpace: 'nowrap',
    [compactHeader]: {
      minWidth: '32px',
      paddingLeft: 0,
      paddingRight: 0,
      '& .fui-Button__icon': { margin: 0 },
    },
  },
  updateLabel: { [compactHeader]: { display: 'none' } },
})

type Common = {
  info: ShaderInfo
  /** O que está escolhido: preset, `curated:<id>` ou caminho de `.slangp`
   *  (`''` = herdar, com `inherit`). */
  selected: string
  /** Troca em andamento: os cards ficam desabilitados. */
  busy?: boolean
  onPick: (name: string) => void
  /** Card "Herdar" (valor `''`) no topo, com esta descrição. */
  inherit?: string
  /** Botão de ajustes no card do shader (só o ativo tem parâmetros). */
  settingsFor?: (id: string) => (() => void) | undefined
  /** Lista na altura que sobrar no pai, em vez da altura máxima fixa. */
  fill?: boolean
}

/** Shaders do ReEmu (embutidos + prontos do pacote) como cards
 *  selecionáveis. Sem GPU ficam todos desabilitados, só pra informar. */
export function ShaderPresetList({
  info,
  selected,
  busy,
  onPick,
  inherit,
  settingsFor,
  fill,
}: Common) {
  const { t } = useTranslation()
  const s = useStyles()
  const off = busy || !info.gpu
  return (
    <div className={mergeClasses(s.list, fill && s.listFill)}>
      {inherit !== undefined && (
        <ShaderCard
          title={t('game.shader.inherit')}
          description={inherit}
          selected={selected === ''}
          disabled={off}
          onSelect={() => onPick('')}
        />
      )}
      {info.available.map((name) => (
        <ShaderCard
          key={name}
          title={presetTitle(t, name)}
          description={presetDesc(t, name)}
          selected={selected === name}
          disabled={off}
          onSelect={() => onPick(name)}
          onSettings={settingsFor?.(name)}
        />
      ))}
      {info.curated.map((c) => (
        <ShaderCard
          key={c.id}
          title={curatedText(t, c.id, 'label', c.label)}
          description={
            c.available
              ? curatedText(t, c.id, 'desc', c.desc)
              : t('video.needsPack', { desc: curatedText(t, c.id, 'desc', c.desc) })
          }
          selected={selected === c.id}
          disabled={off || !c.available}
          onSelect={() => onPick(c.id)}
          onSettings={settingsFor?.(c.id)}
        />
      ))}
    </div>
  )
}

type Source = 'reemu' | 'pack'

/**
 * Escolha de shader: abas "Shaders do ReEmu" (cards dos presets) e "Pacote
 * de shaders" (os `.slangp` do pacote baixado, nos mesmos cards). Usada nas
 * Configurações (shader padrão, ao lado da prévia) e na página do jogo.
 */
export function ShaderPicker(props: Common) {
  const { t } = useTranslation()
  const s = useStyles()
  const tb = useTabStyles()
  const { info, selected } = props
  // Abre na aba do que está escolhido: um caminho de arquivo é do pacote.
  const isPath =
    selected !== '' &&
    !info.available.includes(selected) &&
    !info.curated.some((c) => c.id === selected)
  const [source, setSource] = useState<Source>(isPath ? 'pack' : 'reemu')
  const pack = useShaderPack(source === 'pack')
  const updateLabel = t(pack.downloading ? 'shaders.downloading' : 'shaders.updatePack')

  return (
    <>
      <div className={s.header}>
        <TabList
          className={mergeClasses(tb.tabs, s.tabs)}
          selectedValue={source}
          onTabSelect={(_, d) => setSource(d.value as Source)}
        >
          <Tab value="reemu">{t('video.sourceReemu')}</Tab>
          <Tab value="pack">{t('video.sourcePack')}</Tab>
        </TabList>
        {source === 'pack' && pack.updateAvailable && (
          <Tooltip positioning={TOOLTIP_POSITION} content={updateLabel} relationship="label">
            <Button
              className={s.update}
              appearance="primary"
              icon={pack.downloading ? <Spinner size="tiny" /> : <ArrowSyncRegular />}
              disabledFocusable={pack.downloading}
              onClick={pack.download}
            >
              <span className={s.updateLabel}>{updateLabel}</span>
            </Button>
          </Tooltip>
        )}
      </div>
      {source === 'reemu' ? (
        <ShaderPresetList {...props} />
      ) : (
        <ShaderLibrary
          pack={pack}
          onPick={props.onPick}
          activePath={selected}
          busy={!!props.busy}
          settingsFor={props.settingsFor}
          fill={props.fill}
        />
      )}
    </>
  )
}
