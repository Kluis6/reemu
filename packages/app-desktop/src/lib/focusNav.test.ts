// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  adjustEngaged,
  closeTopmostOverlay,
  cycleTab,
  disengage,
  engage,
  engagedControl,
  isAdjustable,
  moveFocus,
  navCandidates,
  navigateBack,
  navScope,
  topmostModalSurface,
  topmostOverlay,
} from './focusNav'

/** jsdom não faz layout: cada elemento recebe uma caixa em `data-box` (x,y). */
function withBoxes() {
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
    const [x, y] = (this.getAttribute('data-box') ?? '0,0').split(',').map(Number)
    return { left: x, top: y, right: x + 10, bottom: y + 10, width: 10, height: 10, x, y, toJSON() {} }
  })
  Element.prototype.scrollIntoView = () => {}
}

beforeEach(withBoxes)
afterEach(() => {
  disengage()
  vi.restoreAllMocks()
  document.body.innerHTML = ''
})

describe('camada aberta', () => {
  it('o foco não sai do modal para os itens atrás dele', () => {
    document.body.innerHTML = `
      <button id="fora" data-box="0,100">fora</button>
      <div class="fui-DialogSurface" data-box="0,0">
        <button id="dentro" data-box="0,0">dentro</button>
      </div>`
    document.getElementById('dentro')!.focus()
    moveFocus('down')
    expect(document.activeElement?.id).toBe('dentro')
    expect(navCandidates().map((e) => e.id)).toEqual(['dentro'])
  })

  it('a última camada aberta é a de cima', () => {
    document.body.innerHTML = `
      <div class="fui-OverlayDrawer" id="drawer" data-box="0,0"><button data-box="0,0"></button></div>
      <div role="menu" id="menu" data-box="0,0"><button data-box="0,0"></button></div>`
    expect((navScope() as HTMLElement).id).toBe('menu')
  })

  it('sem camada, vale a tela toda', () => {
    document.body.innerHTML = '<button data-box="0,0"></button>'
    expect(navScope()).toBe(document)
  })
})

describe('grupo de navegação', () => {
  it('anda por dentro do grupo antes de sair dele, mesmo com o rodapé mais perto', () => {
    document.body.innerHTML = `
      <div data-nav-group>
        <button id="a" data-box="0,0">a</button>
        <button id="b" data-box="80,900">b</button>
      </div>
      <button id="rodape" data-box="0,400">rodape</button>`
    document.getElementById('a')!.focus()
    moveFocus('down')
    expect(document.activeElement?.id).toBe('b')
    // sem mais nada no grupo, a navegação sai para a tela
    moveFocus('up')
    expect(document.activeElement?.id).toBe('a')
    document.getElementById('b')!.focus()
    moveFocus('up')
    expect(document.activeElement?.id).toBe('a')
  })

  it('sem grupo nada muda: o mais perto ganha', () => {
    document.body.innerHTML = `
      <button id="a" data-box="0,0">a</button>
      <button id="b" data-box="80,900">b</button>
      <button id="rodape" data-box="0,400">rodape</button>`
    document.getElementById('a')!.focus()
    moveFocus('down')
    expect(document.activeElement?.id).toBe('rodape')
  })

  it('o fim do grupo cai no botão de fora', () => {
    document.body.innerHTML = `
      <div data-nav-group><button id="a" data-box="0,0">a</button></div>
      <button id="rodape" data-box="0,400">rodape</button>`
    document.getElementById('a')!.focus()
    moveFocus('down')
    expect(document.activeElement?.id).toBe('rodape')
  })
})

describe('ajuste com o controle', () => {
  it('seletor muda de opção e avisa, sem passar do fim', () => {
    document.body.innerHTML = '<select id="s"><option>a</option><option>b</option></select>'
    const s = document.getElementById('s') as HTMLSelectElement
    const changed = vi.fn()
    s.addEventListener('change', changed)
    expect(isAdjustable(s)).toBe(true)
    engage(s)
    adjustEngaged(1)
    adjustEngaged(1)
    expect(s.value).toBe('b')
    expect(changed).toHaveBeenCalledTimes(1)
  })

  it('perder o foco encerra o ajuste', () => {
    document.body.innerHTML = '<select id="s"><option>a</option></select>'
    const s = document.getElementById('s') as HTMLSelectElement
    s.focus()
    engage(s)
    s.blur()
    expect(engagedControl()).toBeNull()
    expect(s.hasAttribute('data-nav-engaged')).toBe(false)
  })
})

describe('guardas de foco do Fluent', () => {
  it('no fim da lista o foco fica no último item, sem cair na guarda invisível', () => {
    document.body.innerHTML = `
      <div class="fui-DialogSurface" data-box="0,0">
        <button id="ultimo" data-box="0,100">último</button>
        <i tabindex="0" data-tabster-dummy="" data-box="0,110"></i>
      </div>`
    document.getElementById('ultimo')!.focus()
    moveFocus('down')
    expect(document.activeElement?.id).toBe('ultimo')
  })
})

describe('rolagem do foco', () => {
  it('revela o card inteiro quando o foco está no input dele', () => {
    document.body.innerHTML = `
      <button id="a" data-box="0,0">a</button>
      <div class="fui-Card" id="card"><input id="b" data-box="0,100" /></div>`
    const revealed: string[] = []
    Element.prototype.scrollIntoView = function (this: Element) {
      revealed.push(this.id)
    }
    document.getElementById('a')!.focus()
    moveFocus('down')
    expect(document.activeElement?.id).toBe('b')
    expect(revealed).toEqual(['card'])
  })
})

describe('barra lateral', () => {
  it('descer no fim do conteúdo não salta para a barra', () => {
    document.body.innerHTML = `
      <nav data-nav-rail><button id="sino" data-box="0,500"></button></nav>
      <div data-nav-content><button id="card" data-box="100,100"></button></div>`
    document.getElementById('card')!.focus()
    moveFocus('down')
    expect(document.activeElement?.id).toBe('card')
    moveFocus('left')
    expect(document.activeElement?.id).toBe('sino')
  })
})

describe('LB/RB', () => {
  it('troca de aba, dá a volta e pula a desativada', () => {
    document.body.innerHTML = `
      <div role="tablist">
        <button role="tab" id="a" aria-selected="false" data-box="0,0"></button>
        <button role="tab" id="b" aria-selected="false" aria-disabled="true" data-box="0,0"></button>
        <button role="tab" id="c" aria-selected="true" data-box="0,0"></button>
      </div>`
    const clicked: string[] = []
    document.body.addEventListener('click', (e) => clicked.push((e.target as HTMLElement).id))
    expect(cycleTab(1)).toBe(true)
    expect(clicked).toEqual(['a'])
    expect(document.activeElement?.id).toBe('a')
  })

  it('sem abas, não faz nada', () => {
    document.body.innerHTML = '<button data-box="0,0"></button>'
    expect(cycleTab(1)).toBe(false)
  })
})

describe('fechamento de camadas e retorno de rota', () => {
  it('topmostModalSurface retorna a superfície modal do topo e ignora menus leves', () => {
    document.body.innerHTML = `
      <div class="fui-OverlayDrawer" id="drawer" data-box="0,0"></div>
      <div class="fui-DialogSurface" id="dialog" data-box="0,0"></div>
      <div role="menu" id="menu" data-box="0,0"></div>`
    expect(topmostOverlay()?.id).toBe('menu')
    expect(topmostModalSurface()?.id).toBe('dialog')
  })

  it('closeTopmostOverlay clica o botão de fechar da camada do topo sem fechar a de baixo', () => {
    document.body.innerHTML = `
      <div class="fui-OverlayDrawer" id="drawer" data-box="0,0">
        <button id="fechar-drawer" data-overlay-close="" data-box="0,0"></button>
      </div>
      <div class="fui-DialogSurface" id="dialog" data-box="0,0">
        <button id="fechar-dialog" data-overlay-close="" data-box="0,0"></button>
      </div>`
    const clicked: string[] = []
    document.body.addEventListener('click', (e) => clicked.push((e.target as HTMLElement).id))
    expect(closeTopmostOverlay()).toBe(true)
    expect(clicked).toEqual(['fechar-dialog'])
  })

  it('closeTopmostOverlay(true) ignora o drawer de pausa, mas fecha diálogo aberto por cima dele', () => {
    document.body.innerHTML = `
      <div class="fui-OverlayDrawer" id="pausa" data-pause-drawer="" data-box="0,0">
        <button id="fechar-pausa" data-overlay-close="" data-box="0,0"></button>
      </div>`
    expect(closeTopmostOverlay(true)).toBe(false)

    const dlg = document.createElement('div')
    dlg.className = 'fui-DialogSurface'
    dlg.setAttribute('data-box', '0,0')
    dlg.innerHTML = '<button id="fechar-modal" data-overlay-close="" data-box="0,0"></button>'
    document.body.appendChild(dlg)

    const clicked: string[] = []
    document.body.addEventListener('click', (e) => clicked.push((e.target as HTMLElement).id))
    expect(closeTopmostOverlay(true)).toBe(true)
    expect(clicked).toEqual(['fechar-modal'])
  })

  it('navigateBack volta no histórico quando idx > 0 e vai para a tela inicial quando idx === 0', () => {
    const nav = vi.fn()
    window.history.replaceState({ idx: 2 }, '')
    navigateBack(nav)
    expect(nav).toHaveBeenCalledWith(-1)

    nav.mockClear()
    window.history.replaceState({ idx: 0 }, '')
    navigateBack(nav)
    expect(nav).toHaveBeenCalledWith('/', { replace: true })
  })
})
