# 10 — Catálogo e Download de Cores

## Objetivo desta etapa

Implementar a listagem e download de cores a partir do
`buildbot.libretro.com` em tempo real (Abordagem A, decisão tomada —
sem índice/mirror próprio), incluindo cores experimentais.

## Decisões relevantes

- **Leitura ao vivo do buildbot**, não um índice espelhado por vocês —
  isso foi decidido cientes do trade-off (fragilidade a mudanças de
  formato do buildbot). Compense com os pontos abaixo, não reintroduzindo
  um mirror.
- **Isolamento obrigatório**: todo código que sabe o formato da listagem
  do buildbot fica num módulo único (`buildbot_catalog_adapter` ou
  equivalente) atrás da trait de catálogo — se o formato mudar, o
  conserto é local.
- **Cache local com fallback gracioso**: a última listagem bem-sucedida
  fica cacheada (`installed_cores`/tabela de catálogo local); se o fetch
  falhar, mostra o último catálogo conhecido com um aviso (toast), não
  quebra a tela.
- **Timeout agressivo + retry único** no fetch — não deixe a tela de
  cores travada esperando o buildbot responder.
- **Metadata técnica de renderização** (`render_backend` etc.) **não**
  vem do buildbot — é detectada em runtime no primeiro load
  (`02-core-loader-desktop.md`), então a listagem de catálogo mostra essa
  info como "desconhecida até instalar", não tenta adivinhar.
- **Bloqueio pós-instalação pra cores Vulkan-only, antes da fase 2 existir**:
  se `render_backend = vulkan` for detectado no primeiro load e a
  negociação Vulkan por-core ainda não estiver implementada
  (`12-vulkan-hw-render-fase2.md`), não deixe o core falhar de forma
  confusa — exiba explicitamente "requer Vulkan HW render (não suportado
  ainda)" e impeça a execução. O core continua listado/baixável
  normalmente (requisito de listagem completa), só a execução é
  bloqueada.
- **Status experimental**: vem de um arquivo estático versionado no
  próprio repo do app (não do buildbot, não de infraestrutura externa) —
  ver arquivo `core-status.json` (a criar nesta etapa) e unir com o
  catálogo ao vivo por `core_id` na hora de exibir.

## Android

Como decidido em `11-android-port.md`, o Android usa o **mesmo fluxo**
desta etapa (download dinâmico via buildbot), não um catálogo restrito —
só o carregamento em si (`dlopen`) diverge, não a listagem/download.

## Estrutura sugerida

```
crates/core-loader-desktop/src/catalog/
  buildbot_adapter.rs   -- único lugar que conhece o formato do buildbot
  core_status.json        -- curadoria estática de status experimental
  download_manager.rs      -- download, checksum, extração, instalação
```

## Depende de

`01-domain-db.md` (cache local do catálogo).

## Critério de pronto

- Listagem de cores carrega da rede, com fallback funcional se o buildbot
  estiver fora do ar
- Um core experimental aparece marcado como tal na UI, vindo do arquivo
  estático
- Download + instalação de um core funciona ponta a ponta, incluindo
  verificação de checksum
