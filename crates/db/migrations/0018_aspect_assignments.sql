-- Proporção da imagem escolhida pelo usuário, em cascata jogo → plataforma →
-- todos (o `libretro.h` permite ao frontend ignorar o `aspect_ratio` do core).
-- `scope`: 'rom' | 'system' | 'default'; `target`: id da ROM, id do sistema ou
-- '' no padrão. Sem linha = herda (no fim da cascata, a proporção do core).
-- `aspect`: 'core' | '4:3' | '16:9' | 'fill'.
CREATE TABLE aspect_assignments (
    scope TEXT NOT NULL,
    target TEXT NOT NULL DEFAULT '',
    aspect TEXT NOT NULL,
    PRIMARY KEY (scope, target)
);
