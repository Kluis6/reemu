import { Menu, MenuItem, MenuList, MenuPopover, MenuTrigger } from '@fluentui/react-components'
import { useState } from 'react'
import { initials } from '../lib/initials'
import { useCardStyles } from '../styles/xbox'

export interface CardMenuItem {
  label: string
  onClick: () => void
}

/**
 * Cartão de jogo no estilo Xbox: capa em retrato (3:4), badge do sistema,
 * título/subtítulo. Botão ☰ / clique-direito abre o menu de contexto
 * (`menu` — como no dashboard Xbox).
 */
export function GameCard({
  title,
  subtitle,
  badge,
  boxart,
  onClick,
  menu,
}: {
  title: string
  subtitle?: string
  badge?: string
  boxart?: string | null
  onClick?: () => void
  menu?: readonly CardMenuItem[]
}) {
  const s = useCardStyles()
  const [broken, setBroken] = useState(false)
  const showArt = boxart && !broken

  const card = (
    <button className={s.card} onClick={onClick}>
      <div className={s.art} data-art>
        {showArt ? (
          <img src={boxart} alt={title} loading="lazy" onError={() => setBroken(true)} />
        ) : (
          <span style={{ fontSize: 30, opacity: 0.5 }}>{initials(title)}</span>
        )}
        {badge && <span className={s.badge}>{badge}</span>}
      </div>
      <span className={s.title}>{title}</span>
      {subtitle && <span className={s.sub}>{subtitle}</span>}
    </button>
  )

  if (!menu || menu.length === 0) return card

  return (
    <Menu openOnContext>
      <MenuTrigger disableButtonEnhancement>{card}</MenuTrigger>
      <MenuPopover>
        <MenuList>
          {menu.map((m) => (
            <MenuItem key={m.label} onClick={m.onClick}>
              {m.label}
            </MenuItem>
          ))}
        </MenuList>
      </MenuPopover>
    </Menu>
  )
}
