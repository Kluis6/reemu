//! Arquivos de sistema (BIOS) que alguns cores libretro exigem ou aceitam
//! opcionalmente, além da ROM em si. Tabela pura — sem I/O; quem confere o
//! que está de fato no disco é o adapter (`apps/desktop/src-tauri/src/bios.rs`).
//!
//! Convenção de pasta = a mesma do RetroArch: os arquivos ficam em
//! `<system_dir>/[subfolder/]<filename>`, onde `system_dir` é o que o core lê
//! via `RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY`.
//!
//! Nomes de arquivo e MD5 conferidos em `docs.libretro.com/library/<core>/`
//! (não de memória — ver `docs/ai-context/REFERENCES.md`): Beetle PSX,
//! Kronos (Saturn), Flycast (Dreamcast), FBNeo, Genesis Plus GX + PicoDrive
//! (Sega CD), Beetle PCE Fast (PC Engine CD), Beetle PC-FX. Os nomes também
//! batem com as strings dos próprios `.so` desses cores.
//!
//! PSP não entra aqui: o PPSSPP não usa BIOS, e sim a pasta `assets` do
//! projeto PPSSPP (GPL) em `<system_dir>/PPSSPP/` — outro tipo de coisa.

/// Um arquivo de sistema esperado por um core, pra um `system_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiosFile {
    pub filename: &'static str,
    /// Subpasta dentro de `system_dir` (ex: `"dc"`, `"kronos"`, `"fbneo"`).
    /// `None` = direto na raiz de `system_dir`.
    pub subfolder: Option<&'static str>,
    /// MD5s aceitos (qualquer um vale — cores diferentes documentam
    /// revisões diferentes do mesmo arquivo, ex: `bios_CD_U.bin`). Vazio =
    /// só confere presença (ex: arcade, onde cada jogo pede um BIOS
    /// diferente).
    pub md5: &'static [&'static str],
    /// O core recusa rodar sem isso (`true`) ou tem fallback HLE (`false`).
    pub required: bool,
    /// Região/variante ou observação (mostrado na UI).
    pub note: &'static str,
}

const PSX: &[BiosFile] = &[
    BiosFile {
        filename: "scph5500.bin",
        subfolder: None,
        md5: &["8dd7d5296a650fac7319bce665a6a53c"],
        required: false,
        note: "NTSC-J (Japão) — Beetle PSX cai pro OpenBIOS embutido se faltar",
    },
    BiosFile {
        filename: "scph5501.bin",
        subfolder: None,
        md5: &["490f666e1afb15b7362b406ed1cea246"],
        required: false,
        note: "NTSC-U (EUA) — Beetle PSX cai pro OpenBIOS embutido se faltar",
    },
    BiosFile {
        filename: "scph5502.bin",
        subfolder: None,
        md5: &["32736f17079d0b2b7024407c39bd3050"],
        required: false,
        note: "PAL (Europa) — Beetle PSX cai pro OpenBIOS embutido se faltar",
    },
];

const SATURN: &[BiosFile] = &[BiosFile {
    filename: "saturn_bios.bin",
    subfolder: Some("kronos"),
    md5: &["af5828fdff51384f99b3c4926be27762"],
    required: true,
    note: "Kronos NÃO faz HLE — precisa de um BIOS real de Saturn pra rodar",
}];

const DREAMCAST: &[BiosFile] = &[BiosFile {
    filename: "dc_boot.bin",
    subfolder: Some("dc"),
    md5: &["e10c53c2f8b90bab96ead2d368858623"],
    required: false,
    note: "Flycast tem opção \"Enable HLE BIOS\" — roda sem, com menos precisão",
}];

const ARCADE: &[BiosFile] = &[
    BiosFile {
        filename: "neogeo.zip",
        subfolder: Some("fbneo"),
        md5: &[],
        required: false,
        note: "Jogos de Neo Geo (MVS/AES) do FBNeo pedem isso",
    },
    BiosFile {
        filename: "neocdz.zip",
        subfolder: Some("fbneo"),
        md5: &[],
        required: false,
        note: "Neo Geo CD — precisa também do neogeo.zip",
    },
    BiosFile {
        filename: "coleco.zip",
        subfolder: Some("fbneo"),
        md5: &[],
        required: false,
        note: "ColecoVision via FBNeo",
    },
    BiosFile {
        filename: "pgm.zip",
        subfolder: Some("fbneo"),
        md5: &[],
        required: false,
        note: "PGM System (IGS) via FBNeo",
    },
    BiosFile {
        filename: "decocass.zip",
        subfolder: Some("fbneo"),
        md5: &[],
        required: false,
        note: "DECO Cassette System via FBNeo",
    },
    BiosFile {
        filename: "fdsbios.zip",
        subfolder: Some("fbneo"),
        md5: &[],
        required: false,
        note: "Famicom Disk System via FBNeo",
    },
];

const SEGA_CD: &[BiosFile] = &[
    BiosFile {
        filename: "bios_CD_U.bin",
        subfolder: None,
        // 1º: Genesis Plus GX; 2º: PicoDrive — revisões diferentes, as duas
        // documentadas como válidas.
        md5: &[
            "854b9150240a198070150e4566ae1290",
            "2efd74e3232ff260e371b99f84024f7f",
        ],
        required: false,
        note: "Sega CD (EUA) — obrigatório pros jogos dos EUA",
    },
    BiosFile {
        filename: "bios_CD_E.bin",
        subfolder: None,
        md5: &["e66fa1dc5820d254611fdcdba0662372"],
        required: false,
        note: "Mega-CD (Europa) — obrigatório pros jogos europeus",
    },
    BiosFile {
        filename: "bios_CD_J.bin",
        subfolder: None,
        md5: &["278a9397d192149e84e820ac621a8edd"],
        required: false,
        note: "Mega-CD (Japão) — obrigatório pros jogos japoneses",
    },
];

const PC_ENGINE_CD: &[BiosFile] = &[
    BiosFile {
        filename: "syscard3.pce",
        subfolder: None,
        md5: &["38179df8f4ac870017db21ebcbf53114"],
        required: true,
        note: "Super CD-ROM² System 3.x — o padrão dos cores Beetle PCE",
    },
    BiosFile {
        filename: "syscard2.pce",
        subfolder: None,
        md5: &[],
        required: false,
        note: "CD-ROM² System 2.x — alternativa (opção \"CD BIOS\" do core)",
    },
    BiosFile {
        filename: "syscard1.pce",
        subfolder: None,
        md5: &[],
        required: false,
        note: "CD-ROM² System 1.x — alternativa (opção \"CD BIOS\" do core)",
    },
    BiosFile {
        filename: "gexpress.pce",
        subfolder: None,
        md5: &[],
        required: false,
        note: "Game Express CD Card — só pros jogos da Games Express",
    },
];

const PC_FX: &[BiosFile] = &[BiosFile {
    filename: "pcfx.rom",
    subfolder: None,
    md5: &["08e36edbea28a017f79f8d4f7ff9b6d7"],
    required: true,
    note: "BIOS v1.00 — o Beetle PC-FX não roda sem",
}];

/// Arquivos de sistema conhecidos pra um `system_id`. `&[]` = o sistema não
/// precisa de nada além da ROM (a maioria — cartucho puro).
pub fn bios_files_for_system(system_id: &str) -> &'static [BiosFile] {
    match system_id {
        "psx" => PSX,
        "saturn" => SATURN,
        "dreamcast" => DREAMCAST,
        "arcade" => ARCADE,
        "segacd" => SEGA_CD,
        "pcenginecd" => PC_ENGINE_CD,
        "pcfx" => PC_FX,
        _ => &[],
    }
}

/// Todos os `system_id` que este módulo conhece BIOS pra — pra quem quiser
/// varrer tudo de uma vez (ex: painel de Configurações).
pub const KNOWN_SYSTEMS: &[&str] = &[
    "psx",
    "saturn",
    "dreamcast",
    "arcade",
    "segacd",
    "pcenginecd",
    "pcfx",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psx_has_three_region_variants_all_optional() {
        let files = bios_files_for_system("psx");
        assert_eq!(files.len(), 3);
        assert!(files.iter().all(|f| !f.required));
        assert!(files.iter().all(|f| !f.md5.is_empty()));
    }

    #[test]
    fn saturn_bios_is_required() {
        let files = bios_files_for_system("saturn");
        assert_eq!(files.len(), 1);
        assert!(files[0].required);
        assert_eq!(files[0].subfolder, Some("kronos"));
    }

    #[test]
    fn sega_cd_us_accepts_both_documented_revisions() {
        let us = bios_files_for_system("segacd")
            .iter()
            .find(|f| f.filename == "bios_CD_U.bin")
            .unwrap();
        assert_eq!(us.md5.len(), 2);
    }

    #[test]
    fn pc_engine_cd_and_pc_fx_require_their_default_bios() {
        let pce = bios_files_for_system("pcenginecd");
        assert!(pce
            .iter()
            .any(|f| f.filename == "syscard3.pce" && f.required));
        assert_eq!(pce.iter().filter(|f| f.required).count(), 1);
        assert!(bios_files_for_system("pcfx")[0].required);
    }

    #[test]
    fn psp_has_no_bios_entry() {
        assert!(bios_files_for_system("psp").is_empty());
    }

    #[test]
    fn documented_md5s_are_lowercase_hex() {
        for sys in KNOWN_SYSTEMS {
            for f in bios_files_for_system(sys) {
                for h in f.md5 {
                    assert_eq!(h.len(), 32, "{sys}/{}", f.filename);
                    assert!(
                        h.chars()
                            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
                        "{sys}/{}",
                        f.filename
                    );
                }
            }
        }
    }

    #[test]
    fn unknown_system_has_no_bios() {
        assert_eq!(bios_files_for_system("nes"), &[] as &[BiosFile]);
        assert_eq!(bios_files_for_system("whatever"), &[] as &[BiosFile]);
    }

    #[test]
    fn known_systems_all_resolve_to_non_empty() {
        for sys in KNOWN_SYSTEMS {
            assert!(
                !bios_files_for_system(sys).is_empty(),
                "{sys} devia ter pelo menos 1 BiosFile"
            );
        }
    }
}
