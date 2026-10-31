// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  adjustEngaged,
  cycleTab,
  disengage,
  engage,
  engagedControl,
  isAdjustable,
  moveFocus,
  navCandidates,
  navScope,
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
