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
//! Kronos (Saturn), Flycast (Dreamcast), FBNeo.

/// Um arquivo de sistema esperado por um core, pra um `system_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiosFile {
    pub filename: &'static str,
    /// Subpasta dentro de `system_dir` (ex: `"dc"`, `"kronos"`, `"fbneo"`).
    /// `None` = direto na raiz de `system_dir`.
    pub subfolder: Option<&'static str>,
    /// MD5 do arquivo oficial, quando documentado. `None` = só confere
    /// presença (ex: arcade, onde cada jogo pede um BIOS diferente).
    pub md5: Option<&'static str>,
    /// O core recusa rodar sem isso (`true`) ou tem fallback HLE (`false`).
    pub required: bool,
    /// Região/variante ou observação (mostrado na UI).
    pub note: &'static str,
}

const PSX: &[BiosFile] = &[
    BiosFile {
        filename: "scph5500.bin",
        subfolder: None,
        md5: Some("8dd7d5296a650fac7319bce665a6a53c"),
        required: false,
        note: "NTSC-J (Japão) — Beetle PSX cai pro OpenBIOS embutido se faltar",
    },
    BiosFile {
        filename: "scph5501.bin",
        subfolder: None,
        md5: Some("490f666e1afb15b7362b406ed1cea246"),
        required: false,
        note: "NTSC-U (EUA) — Beetle PSX cai pro OpenBIOS embutido se faltar",
    },
    BiosFile {
        filename: "scph5502.bin",
        subfolder: None,
        md5: Some("32736f17079d0b2b7024407c39bd3050"),
        required: false,
        note: "PAL (Europa) — Beetle PSX cai pro OpenBIOS embutido se faltar",
    },
];

const SATURN: &[BiosFile] = &[BiosFile {
    filename: "saturn_bios.bin",
    subfolder: Some("kronos"),
    md5: Some("af5828fdff51384f99b3c4926be27762"),
    required: true,
    note: "Kronos NÃO faz HLE — precisa de um BIOS real de Saturn pra rodar",
}];

const DREAMCAST: &[BiosFile] = &[BiosFile {
    filename: "dc_boot.bin",
    subfolder: Some("dc"),
    md5: Some("e10c53c2f8b90bab96ead2d368858623"),
    required: false,
    note: "Flycast tem opção \"Enable HLE BIOS\" — roda sem, com menos precisão",
}];

const ARCADE: &[BiosFile] = &[
    BiosFile {
        filename: "neogeo.zip",
        subfolder: Some("fbneo"),
        md5: None,
        required: false,
        note: "Jogos de Neo Geo (MVS/AES) do FBNeo pedem isso",
    },
    BiosFile {
        filename: "neocdz.zip",
        subfolder: Some("fbneo"),
        md5: None,
        required: false,
        note: "Neo Geo CD — precisa também do neogeo.zip",
    },
    BiosFile {
        filename: "coleco.zip",
        subfolder: Some("fbneo"),
        md5: None,
        required: false,
        note: "ColecoVision via FBNeo",
    },
    BiosFile {
        filename: "pgm.zip",
        subfolder: Some("fbneo"),
        md5: None,
        required: false,
        note: "PGM System (IGS) via FBNeo",
    },
    BiosFile {
        filename: "decocass.zip",
        subfolder: Some("fbneo"),
        md5: None,
        required: false,
        note: "DECO Cassette System via FBNeo",
    },
    BiosFile {
        filename: "fdsbios.zip",
        subfolder: Some("fbneo"),
        md5: None,
        required: false,
        note: "Famicom Disk System via FBNeo",
    },
];

/// Arquivos de sistema conhecidos pra um `system_id`. `&[]` = o sistema não
/// precisa de nada além da ROM (a maioria — cartucho puro).
pub fn bios_files_for_system(system_id: &str) -> &'static [BiosFile] {
    match system_id {
        "psx" => PSX,
        "saturn" => SATURN,
        "dreamcast" => DREAMCAST,
        "arcade" => ARCADE,
        _ => &[],
    }
}

/// Todos os `system_id` que este módulo conhece BIOS pra — pra quem quiser
/// varrer tudo de uma vez (ex: painel de Configurações).
pub const KNOWN_SYSTEMS: &[&str] = &["psx", "saturn", "dreamcast", "arcade"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psx_has_three_region_variants_all_optional() {
        let files = bios_files_for_system("psx");
        assert_eq!(files.len(), 3);
        assert!(files.iter().all(|f| !f.required));
        assert!(files.iter().all(|f| f.md5.is_some()));
    }

    #[test]
    fn saturn_bios_is_required() {
        let files = bios_files_for_system("saturn");
        assert_eq!(files.len(), 1);
        assert!(files[0].required);
        assert_eq!(files[0].subfolder, Some("kronos"));
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
