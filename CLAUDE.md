# Instruções do projeto ReEmu

Comece por [`TASKS.md`](TASKS.md) (o que está em aberto) e
[`docs/historico.md`](docs/historico.md) (o que foi feito e por quê). O
contexto de cada etapa está em `docs/ai-context/`.

## Regra: OpenGL, Vulkan e GPU só com a documentação oficial

Sempre que o trabalho tocar OpenGL, Vulkan, WGL, EGL, GLX, wgpu, shaders ou
qualquer API de GPU, **consulte a documentação oficial antes de decidir ou
escrever código** — não implemente de memória. Confira nela os valores de
constantes, a ordem das chamadas, os requisitos de sincronização e o que a
especificação garante (ou não) em cada plataforma. Registre no commit ou no
`docs/historico.md` qual documento embasou a decisão.

Fontes oficiais, nesta ordem de preferência:

- **Especificações e extensões Khronos** (OpenGL, OpenGL ES, EGL, WGL, GLX,
  Vulkan): <https://registry.khronos.org/> — ex.:
  `OpenGL/extensions/ARB/WGL_ARB_create_context.txt`,
  `EGL/extensions/EXT/EGL_EXT_image_dma_buf_import.txt`.
- **Vulkan**: especificação e guia em <https://docs.vulkan.org/> e
  <https://registry.khronos.org/vulkan/>.
- **OpenGL (páginas de referência)**: <https://registry.khronos.org/OpenGL-Refpages/>.
- **wgpu**: <https://docs.rs/wgpu> (a versão usada está no `Cargo.lock`) e
  <https://www.w3.org/TR/webgpu/> para a semântica.
- **Windows (WGL, DXGI, Direct3D)**: <https://learn.microsoft.com/windows/win32/>.
- **libretro (HW render, `retro_hw_render_callback`, interface Vulkan)**:
  `libretro.h` e `libretro_vulkan.h` do repositório oficial
  <https://github.com/libretro/libretro-common>.
- **Linux (DRM/GBM/dma_buf)**: <https://docs.kernel.org/gpu/> e as páginas
  de manual do Mesa.

Se a documentação oficial não cobrir o caso, diga isso explicitamente e
marque a decisão como não verificada em vez de apresentá-la como certa.
