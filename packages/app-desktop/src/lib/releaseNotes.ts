/**
 * Markdown mínimo das notas de Release (`scripts/release-notes.sh` gera
 * `## Seção` + `- item`) → blocos pra renderizar como elementos React, sem
 * `innerHTML`. Qualquer outra linha vira parágrafo; `**`/`` ` `` somem.
 */
export type NotesBlock =
  | { type: "heading"; text: string }
  | { type: "list"; items: string[] }
  | { type: "paragraph"; text: string };

const clean = (s: string) => s.replace(/\*\*|__|`/g, "").trim();

export function parseReleaseNotes(md: string | null | undefined): NotesBlock[] {
  const blocks: NotesBlock[] = [];
  for (const raw of (md ?? "").split(/\r?\n/)) {
    const line = raw.trim();
    if (!line) continue;
    const heading = line.match(/^#{1,6}\s+(.*)$/);
    const item = line.match(/^[-*+]\s+(.*)$/);
    if (heading) {
      blocks.push({ type: "heading", text: clean(heading[1]) });
    } else if (item) {
      const last = blocks[blocks.length - 1];
      if (last?.type === "list") last.items.push(clean(item[1]));
      else blocks.push({ type: "list", items: [clean(item[1])] });
    } else {
      blocks.push({ type: "paragraph", text: clean(line) });
    }
  }
  return blocks;
}
