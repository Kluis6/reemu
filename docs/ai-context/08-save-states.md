# 08 — Save States e Save RAM

## Objetivo desta etapa

Implementar `SaveStateManager`, cobrindo save state (snapshot via
`retro_serialize`) e save RAM (battery save via
`retro_get_memory_data(RETRO_MEMORY_SAVE_RAM)`) — são entidades distintas,
não trate como a mesma coisa.

## Decisões relevantes

- **Timing**: save acontece imediatamente entre frames, nunca no meio de
  um `retro_run` — o hotkey de quick save só marca uma flag; o loop
  principal (em `02-core-loader-desktop.md`/`03-tauri-desktop-shell.md`)
  checa essa flag logo após completar o `retro_run` do frame atual, antes
  do próximo começar. Não pause o core pra salvar (decisão: Abordagem A,
  já rejeitamos a alternativa de pausar).
- **Armazenamento**: o arquivo de state (binário, pode ser grande) vai pro
  disco — nunca como BLOB no SQLite. O banco só guarda o `file_path` e
  metadata (`08-save-states` schema em `crates/db/migrations/0001_init.sql`).
- **Thumbnail**: capture o frame exato no mesmo instante do
  `retro_serialize`, não um frame antes/depois.
- **`core_id` é obrigatório**: um save state não é portável entre cores
  diferentes do mesmo sistema (formato interno de serialização é
  específico do core). Ao carregar um state, valide que o core ativo
  bate com o `core_id` salvo — se não bater, bloqueie o load com uma
  mensagem clara, não tente carregar mesmo assim.

## Estrutura sugerida

```
crates/core-loader-desktop/src/
  save_state.rs   -- chama retro_serialize/retro_unserialize no momento certo,
                       grava arquivo, delega metadata pro repositório (etapa 01)
```

## Depende de

`02-core-loader-desktop.md` (precisa do core carregado e do loop
principal) e `01-domain-db.md` (repositório de metadata).

## Critério de pronto

- Salvar durante gameplay não causa stutter perceptível
- Carregar um save state de um core diferente do que gerou é bloqueado
  com mensagem clara, não crash
- Thumbnail do save bate visualmente com o momento exato do save
