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
- **Vulkan** (a partir de <https://www.vulkan.org/>, indicado pelo dono do
  projeto): especificação em <https://docs.vulkan.org/spec/latest/index.html>,
  Vulkan Guide <https://github.com/KhronosGroup/Vulkan-Guide>, exemplos
  oficiais <https://github.com/KhronosGroup/Vulkan-Samples> e o registro
  <https://registry.khronos.org/vulkan/>.
- **OpenGL** (a partir de <https://www.opengl.org/>, indicado pelo dono do
  projeto): registro com as especificações de OpenGL e GLSL
  <https://registry.khronos.org/OpenGL/index_gl.php>, páginas de referência
  <https://registry.khronos.org/OpenGL-Refpages/gl4/> (e `gl2.1/` para o
  perfil antigo) e o OpenGL Wiki <https://www.khronos.org/opengl/wiki/>.
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
