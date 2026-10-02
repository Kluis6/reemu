# Avisos de terceiros

O ReEmu é distribuído sob a licença MIT (ver `LICENSE`). Ele inclui
declarações derivadas dos arquivos abaixo, cujos avisos de copyright e
permissão são reproduzidos aqui, como as licenças deles pedem.

## libretro API (`libretro.h`)

Origem: <https://github.com/libretro/libretro-common/blob/master/include/libretro.h>.
Usado em `crates/core-loader-desktop/src/sys.rs` e `vk_sys.rs`.

```
Copyright (C) 2010-2024 The RetroArch team

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## libretro Vulkan API (`libretro_vulkan.h`)

Origem: <https://github.com/libretro/libretro-common/blob/master/include/libretro_vulkan.h>.
Usado em `crates/core-loader-desktop/src/vk_sys.rs`.

```
Copyright (C) 2010-2020 The RetroArch team

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Cores libretro, shaders, molduras e BIOS

O ReEmu **não inclui** cores, shaders, molduras nem BIOS. Os cores são
baixados pelo próprio usuário, sob demanda, do buildbot oficial da libretro
(<https://buildbot.libretro.com>); shaders e molduras, do buildbot e do
GitHub dos projetos de origem. Cada um segue a sua licença (GPL, LGPL, MIT,
MPL ou licenças não comerciais), mostrada no catálogo de cores do app.

Os cores de licença não comercial (por exemplo Genesis Plus GX, Snes9x,
FB Alpha/FBNeo e versões antigas do MAME) "may not be sold, nor may they be
used in a commercial product or activity without copyright holders'
approval" (<https://docs.libretro.com/development/licenses/>).
