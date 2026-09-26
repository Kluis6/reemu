-- Teclado configurável: só o que o usuário mudou por cima do padrão
-- (`input_desktop::keymap::DEFAULTS`). `target` é o nome estável do alvo
-- (`Up`, `R2`, `LStickLeft`…); `code` é o `KeyboardEvent.code` da tecla, ou
-- '' para "sem tecla".
CREATE TABLE keyboard_bindings (
    target TEXT PRIMARY KEY,
    code   TEXT NOT NULL
);
