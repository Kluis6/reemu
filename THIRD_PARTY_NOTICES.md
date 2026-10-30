# Avisos de terceiros

O ReEmu é distribuído sob a licença MIT (ver `LICENSE`). Ele inclui
declarações derivadas dos arquivos abaixo, cujos avisos de copyright e
permissão são reproduzidos aqui, como as licenças deles pedem.

No app, o resumo destas licenças e das bibliotecas usadas fica em
Configurações › Sistema › Informações e licenças. A contagem de bibliotecas
por licença vem de `node scripts/licenses-summary.mjs`
(`packages/app-desktop/src/data/licenses.json`); rode de novo ao mudar
dependências.

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

O ReEmu **não inclui** cores, molduras nem BIOS; dos shaders, inclui só os
próprios e o CAS da AMD (seção abaixo). Os cores são
baixados pelo próprio usuário, sob demanda, do buildbot oficial da libretro
(<https://buildbot.libretro.com>); shaders e molduras, do buildbot e do
GitHub dos projetos de origem. Cada um segue a sua licença (GPL, LGPL, MIT,
MPL ou licenças não comerciais), mostrada no catálogo de cores do app.

Os cores de licença não comercial (por exemplo Genesis Plus GX, Snes9x,
FB Alpha/FBNeo e versões antigas do MAME) "may not be sold, nor may they be
used in a commercial product or activity without copyright holders'
approval" (<https://docs.libretro.com/development/licenses/>).

## AMD FidelityFX CAS (`ffx_cas.h`)

Origem: <https://github.com/GPUOpen-Effects/FidelityFX-CAS/blob/master/ffx-cas/ffx_cas.h>.
Portado (caminho só de nitidez) em `apps/desktop/src-tauri/shaders/cas/cas.slang`.

```
Copyright (c) 2017-2019 Advanced Micro Devices, Inc. All rights reserved.

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software is furnished to do so,
subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN
AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```


## Selawik (fonte)

Origem: <https://github.com/microsoft/Selawik> (release 1.01). Arquivos
`.woff2` sem modificação em `packages/app-desktop/src/assets/fonts/selawik/`,
com a licença completa em `OFL.txt` ao lado. Usada só quando a Segoe UI não
existe no sistema (ex.: Linux).

```
Copyright 2015, Microsoft Corporation (www.microsoft.com), with Reserved Font
Name Selawik. All Rights Reserved. Selawik is a trademark of Microsoft
Corporation in the United States and/or other countries.

This Font Software is licensed under the SIL Open Font License, Version 1.1.
```

## SDL 3 (controles no Windows)

Origem: <https://github.com/libsdl-org/SDL> (3.4.18, via os crates `sdl3-sys`
e `sdl3-src`). Compilado do fonte e ligado estático no app do Windows, só com
os subsistemas de joystick/gamepad e HIDAPI (`crates/input-desktop`).

```
Copyright (C) 1997-2026 Sam Lantinga <slouken@libsdl.org>

This software is provided 'as-is', without any express or implied
warranty.  In no event will the authors be held liable for any damages
arising from the use of this software.

Permission is granted to anyone to use this software for any purpose,
including commercial applications, and to alter it and redistribute it
freely, subject to the following restrictions:

1. The origin of this software must not be misrepresented; you must not
   claim that you wrote the original software. If you use this software
   in a product, an acknowledgment in the product documentation would be
   appreciated but is not required.
2. Altered source versions must be plainly marked as such, and must not be
   misrepresented as being the original software.
3. This notice may not be removed or altered from any source distribution.

```

### HIDAPI (dentro do SDL)

O HIDAPI pode ser usado sob GPLv3, uma licença estilo BSD ou a licença
original dele, "at the discretion of the user" (`src/hidapi/LICENSE.txt`). O
ReEmu usa a estilo BSD:

```
Copyright (c) 2010, Alan Ott, Signal 11 Software
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

    * Redistributions of source code must retain the above copyright notice,
      this list of conditions and the following disclaimer.
    * Redistributions in binary form must reproduce the above copyright
      notice, this list of conditions and the following disclaimer in the
      documentation and/or other materials provided with the distribution.
    * Neither the name of Signal 11 Software nor the names of its
      contributors may be used to endorse or promote products derived from
      this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
POSSIBILITY OF SUCH DAMAGE.
```

## Símbolo do Alvanista (`alvanista-symbol.svg`)

Origem: <https://www.alvanista.com/assets/images/logo/alvanista-symbol.svg>,
copiado para `packages/app-desktop/src/assets/alvanista-symbol.svg`. Aparece
só no botão "Entrar com Alvanista" (Configurações › Perfil).

O símbolo é marca da Tellurian Corp / Alvanista, usada com permissão apenas
nesse botão. **Não** é coberto pela licença MIT do ReEmu.
