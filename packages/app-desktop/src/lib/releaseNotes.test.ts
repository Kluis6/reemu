import { describe, expect, it } from "vitest";
import { parseReleaseNotes } from "./releaseNotes";

describe("parseReleaseNotes", () => {
  it("separa seções, itens e parágrafos", () => {
    const md =
      "## Novidades\n\n- canvas com **WebGL**\n- sino\n\n## Correções\n\n- `to_rgba8`\n\nMudanças desde v0.1.0-rc2.";
    expect(parseReleaseNotes(md)).toEqual([
      { type: "heading", text: "Novidades" },
      { type: "list", items: ["canvas com WebGL", "sino"] },
      { type: "heading", text: "Correções" },
      { type: "list", items: ["to_rgba8"] },
      { type: "paragraph", text: "Mudanças desde v0.1.0-rc2." },
    ]);
  });

  it("aceita vazio/nulo", () => {
    expect(parseReleaseNotes(null)).toEqual([]);
    expect(parseReleaseNotes("\n\n")).toEqual([]);
  });
});
