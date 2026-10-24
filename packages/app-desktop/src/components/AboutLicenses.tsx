import {
  Accordion,
  AccordionHeader,
  AccordionItem,
  AccordionPanel,
  Body1,
  Caption1,
  Card,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
// Texto da licença do ReEmu, embutido do arquivo da raiz do repositório.
import licenseText from '../../../../LICENSE?raw'
import licenses from '../data/licenses.json'
import { appVersion } from '../lib/tauri'

const useStyles = makeStyles({
  root: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fill, minmax(380px, 1fr))',
    gap: tokens.spacingVerticalL,
  },
  // sem a sombra do `filled-alternative` (pedido do usuário, como os cards
  // de Sistema › Opções)
  card: {
    display: 'flex',
    flexDirection: 'column',
    rowGap: tokens.spacingVerticalS,
    boxShadow: 'none',
  },
  wide: { gridColumn: '1 / -1' },
  muted: { color: tokens.colorNeutralForeground3 },
  p: { margin: 0 },
  // nome à esquerda, licença à direita
  list: {
    display: 'grid',
    gridTemplateColumns: 'minmax(140px, max-content) minmax(0, 1fr)',
    columnGap: tokens.spacingHorizontalXXL,
    rowGap: tokens.spacingVerticalXS,
    margin: 0,
  },
  dd: { margin: 0, overflowWrap: 'anywhere' },
  cols: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
    gap: tokens.spacingHorizontalXXL,
  },
  licenseText: {
    margin: 0,
    whiteSpace: 'pre-wrap',
    fontFamily: tokens.fontFamilyMonospace,
    fontSize: tokens.fontSizeBase200,
    color: tokens.colorNeutralForeground2,
  },
})

// Principais bibliotecas que vão no app instalado, com a licença de cada uma
// (conferida no `cargo metadata` / `package.json`; o glslang no LICENSE.txt
// dele). Nomes próprios: não passam pela tradução.
const MAIN_LIBS: [string, string][] = [
  ['Tauri', 'Apache-2.0 / MIT'],
  ['wgpu, naga', 'MIT / Apache-2.0'],
  ['glslang (Khronos)', 'BSD-3-Clause, BSD-2-Clause, MIT, Apache-2.0'],
  ['SDL 3 (Windows)', 'Zlib'],
  ['gilrs (Linux)', 'Apache-2.0 / MIT'],
  ['cpal', 'Apache-2.0'],
  ['React', 'MIT'],
  ['Fluent UI', 'MIT'],
  ['TanStack Query', 'MIT'],
  ['i18next', 'MIT'],
]

/** Quantas licenças aparecem por linguagem; o resto vira "Outras". */
const TOP = 5

const pretty = (expr: string) => expr.replaceAll(' OR ', ' / ')

function Section({
  title,
  wide,
  children,
}: {
  title: string
  wide?: boolean
  children: ReactNode
}) {
  const s = useStyles()
  return (
    <Card appearance="filled-alternative" className={wide ? `${s.card} ${s.wide}` : s.card}>
      <Text as="h3" weight="semibold" className={s.p}>
        {title}
      </Text>
      {children}
    </Card>
  )
}

function Rows({ rows }: { rows: [string, ReactNode][] }) {
  const s = useStyles()
  return (
    <dl className={s.list}>
      {rows.map(([k, v]) => (
        <div key={k} style={{ display: 'contents' }}>
          <dt>
            <Caption1>{k}</Caption1>
          </dt>
          <dd className={s.dd}>
            <Caption1 className={s.muted}>{v}</Caption1>
          </dd>
        </div>
      ))}
    </dl>
  )
}

/**
 * Configurações › Sistema › Informações e licenças: num lugar só, a licença
 * do ReEmu e o resumo do que é de terceiros (cores, shaders, molduras, capas
 * e metadados, bibliotecas). O texto completo dos avisos fica em
 * THIRD_PARTY_NOTICES.md; a contagem das bibliotecas vem de
 * `scripts/licenses-summary.mjs` (src/data/licenses.json).
 */
export function AboutLicenses() {
  const { t } = useTranslation()
  const s = useStyles()
  const version = useQuery({
    queryKey: ['app-version'],
    queryFn: appVersion,
    staleTime: Infinity,
    retry: false,
  })

  const byLang = (data: { byLicense: Record<string, number> }): [string, ReactNode][] => {
    const all = Object.entries(data.byLicense)
    const rest = all.slice(TOP).reduce((n, [, c]) => n + c, 0)
    const rows: [string, ReactNode][] = all.slice(0, TOP).map(([lic, n]) => [pretty(lic), n])
    if (rest > 0) rows.push([t('about.libs.others'), rest])
    return rows
  }

  return (
    <div className={s.root}>
      <Section title="ReEmu" wide>
        <Body1 as="p" className={s.p}>
          {version.data && `${t('about.reemu.version', { version: version.data })} · `}
          {t('about.reemu.license')}
        </Body1>
        <Caption1 className={s.muted}>{t('about.reemu.source')}</Caption1>
        <Accordion collapsible>
          <AccordionItem value="text">
            <AccordionHeader>{t('about.reemu.showText')}</AccordionHeader>
            <AccordionPanel>
              <pre className={s.licenseText}>{licenseText.trim()}</pre>
            </AccordionPanel>
          </AccordionItem>
        </Accordion>
      </Section>

      <Section title={t('about.cores.title')}>
        <Body1 as="p" className={s.p}>
          {t('about.cores.body')}
        </Body1>
        <Caption1 className={s.muted}>{t('about.cores.nonCommercial')}</Caption1>
      </Section>

      <Section title={t('about.shaders.title')}>
        <Body1 as="p" className={s.p}>
          {t('about.shaders.builtin')} {t('about.shaders.pack')}
        </Body1>
        <Caption1 className={s.muted}>{t('about.shaders.megaBezel')}</Caption1>
      </Section>

      <Section title={t('about.bezels.title')}>
        <Body1 as="p" className={s.p}>
          {t('about.bezels.body')}
        </Body1>
      </Section>

      <Section title={t('about.data.title')}>
        <Body1 as="p" className={s.p}>
          {t('about.data.body')}
        </Body1>
      </Section>

      <Section title={t('about.libs.title')} wide>
        <div className={s.cols}>
          <div className={s.card}>
            <Body1 as="p" className={s.p}>
              {t('about.libs.main')}
            </Body1>
            <Rows
              rows={[
                ...MAIN_LIBS,
                ['SQLite', t('about.libs.publicDomain')],
                [t('about.libs.selawik'), 'OFL-1.1 (© Microsoft)'],
                ['libretro.h, libretro_vulkan.h', 'MIT (© The RetroArch team)'],
              ]}
            />
          </div>
          <div className={s.card}>
            <Body1 as="p" className={s.p}>
              {t('about.libs.total', { rust: licenses.rust.total, js: licenses.js.total })}
            </Body1>
            <Text weight="semibold">Rust</Text>
            <Rows rows={byLang(licenses.rust)} />
            <Text weight="semibold">JavaScript</Text>
            <Rows rows={byLang(licenses.js)} />
          </div>
        </div>
      </Section>

      <Section title={t('about.trademarks.title')} wide>
        <Body1 as="p" className={s.p}>
          {t('about.trademarks.body')}
        </Body1>
      </Section>
    </div>
  )
}
