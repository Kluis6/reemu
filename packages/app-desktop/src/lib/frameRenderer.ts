/**
 * Desenha o frame do jogo no `<canvas>` (modo sem surface nativa — o padrão
 * no Windows). Duas implementações com a mesma interface:
 *
 * - **WebGL** (preferida): o frame sobe como textura (`texSubImage2D` quando o
 *   tamanho não muda) e a GPU desenha um quad com filtro *nearest*. Tira da
 *   CPU a cópia/conversão que o `putImageData` faz a cada quadro.
 * - **2D** (fallback): o `putImageData` de antes. Usado quando não há WebGL —
 *   ex.: WebKitGTK com compositing desligado (NVIDIA proprietário, ver
 *   `main.rs`), em que o WebGL pode nem existir.
 *
 * Em ambos o canvas fica no tamanho NATIVO do frame e o CSS amplia com
 * `image-rendering: pixelated` — mesmo visual de antes.
 */

export interface FrameRenderer {
  readonly kind: 'webgl' | '2d'
  /** `pixels`: RGBA8 apertado, `w*h*4` bytes, linha 0 = topo. */
  draw(pixels: Uint8Array<ArrayBuffer>, w: number, h: number): void
  dispose(): void
}

const VS = `
attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
  // quad em clip space; a linha 0 do frame (topo) vai pra v = 0 — o
  // texImage2D sobe a linha 0 em t = 0, então sem flip a imagem sai certa
  // com o topo em cima.
  v_uv = vec2((a_pos.x + 1.0) * 0.5, (1.0 - a_pos.y) * 0.5);
  gl_Position = vec4(a_pos, 0.0, 1.0);
}`

const FS = `
precision mediump float;
varying vec2 v_uv;
uniform sampler2D u_tex;
void main() { gl_FragColor = texture2D(u_tex, v_uv); }`

function compile(gl: WebGLRenderingContext, type: number, src: string): WebGLShader | null {
  const sh = gl.createShader(type)
  if (!sh) return null
  gl.shaderSource(sh, src)
  gl.compileShader(sh)
  if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
    console.warn('frameRenderer: shader não compilou', gl.getShaderInfoLog(sh))
    gl.deleteShader(sh)
    return null
  }
  return sh
}

/** `null` se não houver WebGL ou algo da inicialização falhar. */
function createWebGl(canvas: HTMLCanvasElement): FrameRenderer | null {
  const gl = canvas.getContext('webgl', {
    alpha: false,
    antialias: false,
    depth: false,
    stencil: false,
    premultipliedAlpha: false,
    // ninguém lê os pixels do canvas (o fundo do menu de pausa vem do Rust)
    preserveDrawingBuffer: false,
  }) as WebGLRenderingContext | null
  if (!gl) return null

  let tex: WebGLTexture | null = null
  let texW = 0
  let texH = 0
  let ready = false

  // Monta programa, quad e textura — de novo quando a GPU devolve o contexto
  // depois de perdê-lo (troca de driver, suspensão).
  const setup = (): boolean => {
    const vs = compile(gl, gl.VERTEX_SHADER, VS)
    const fs = compile(gl, gl.FRAGMENT_SHADER, FS)
    if (!vs || !fs) return false
    const prog = gl.createProgram()
    if (!prog) return false
    gl.attachShader(prog, vs)
    gl.attachShader(prog, fs)
    gl.linkProgram(prog)
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return false
    gl.useProgram(prog)

    const buf = gl.createBuffer()
    gl.bindBuffer(gl.ARRAY_BUFFER, buf)
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW)
    const loc = gl.getAttribLocation(prog, 'a_pos')
    gl.enableVertexAttribArray(loc)
    gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0)

    tex = gl.createTexture()
    gl.bindTexture(gl.TEXTURE_2D, tex)
    // textura não potência de 2 no WebGL1: só com CLAMP e sem mipmap
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST)
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST)
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE)
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE)
    // linhas de largura ímpar (ex.: 1 px) não são alinhadas a 4 bytes
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1)
    texW = 0
    texH = 0
    return true
  }

  if (!setup()) return null
  ready = true

  const onLost = (e: Event) => {
    e.preventDefault() // sem isto o navegador não tenta devolver o contexto
    ready = false
  }
  const onRestored = () => {
    ready = setup()
  }
  canvas.addEventListener('webglcontextlost', onLost)
  canvas.addEventListener('webglcontextrestored', onRestored)

  return {
    kind: 'webgl',
    draw(pixels, w, h) {
      if (!ready) return
      if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w
        canvas.height = h
      }
      if (w !== texW || h !== texH) {
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, pixels)
        texW = w
        texH = h
      } else {
        gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, pixels)
      }
      gl.viewport(0, 0, w, h)
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4)
    },
    dispose() {
      canvas.removeEventListener('webglcontextlost', onLost)
      canvas.removeEventListener('webglcontextrestored', onRestored)
    },
  }
}

function create2d(canvas: HTMLCanvasElement): FrameRenderer | null {
  const ctx = canvas.getContext('2d')
  if (!ctx) return null
  return {
    kind: '2d',
    draw(pixels, w, h) {
      if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w
        canvas.height = h
      }
      // view sobre o mesmo buffer — `ImageData` aceita Uint8ClampedArray
      const view = new Uint8ClampedArray(pixels.buffer, pixels.byteOffset, w * h * 4)
      ctx.putImageData(new ImageData(view, w, h), 0, 0)
    },
    dispose() {},
  }
}

/**
 * WebGL se der; senão 2D. `null` só se o canvas não tiver contexto nenhum.
 * Um canvas só aceita UM tipo de contexto — por isso a escolha acontece uma
 * vez, na criação, e não quadro a quadro.
 */
export function createFrameRenderer(canvas: HTMLCanvasElement): FrameRenderer | null {
  return createWebGl(canvas) ?? create2d(canvas)
}
