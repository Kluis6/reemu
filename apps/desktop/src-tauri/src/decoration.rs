//! Importação de um pacote de decoração (bezels) e resolução em runtime.
//! O scan puro fica em `library_scan::scan_decoration_pack`; aqui a gente
//! mapeia os stems de ROM pra `rom_id` (contra o banco) e persiste.

use std::path::Path;

use domain::decoration::{
    DecorationAssignment, DecorationPack, DecorationPackSource, DecorationStore,
};
use domain::library::RomRepository;
use domain::shader_chain::AssignmentScope;
use library_scan::{scan_decoration_pack, DecoScope};

/// Importa o pacote em `path` → `decoration_packs` + `decoration_assignments`.
/// Devolve quantas atribuições foram gravadas.
pub async fn import_pack(pool: &db::Db, path: &Path) -> Result<usize, String> {
    if !path.is_dir() {
        return Err("escolha a pasta do pacote de bezels".into());
    }
    let scanned = scan_decoration_pack(path);
    log::info!(
        "decoração: {} imagem(ns) reconhecida(s) em {}",
        scanned.len(),
        path.display()
    );
    if scanned.is_empty() {
        return Err(format!(
            "nenhum .png de bezel reconhecido em '{}' (nem em subpastas) — \
             aponte a pasta raiz do pacote",
            path.display()
        ));
    }

    let roms = db::RomsRepo::new(pool.clone())
        .list()
        .await
        .map_err(|e| e.to_string())?;
    // stem minúsculo → [(system_id, rom_id)]
    let mut by_stem: std::collections::HashMap<String, Vec<(String, String)>> =
        std::collections::HashMap::new();
    for r in &roms {
        if let Some(stem) = Path::new(&r.file_path).file_stem().and_then(|s| s.to_str()) {
            by_stem
                .entry(stem.to_lowercase())
                .or_default()
                .push((r.system_id.clone(), r.id.clone()));
        }
    }
    // casa um bezel de jogo → rom_id(s). Devolve TODAS as linhas de ROM que
    // batem (a biblioteca costuma ter o mesmo jogo duplicado — dois pontos de
    // montagem do mesmo drive, cópias em pastas diferentes — e o launcher pode
    // abrir qualquer uma delas; se só a 1ª recebe bezel, o jogo abre "sem
    // decoração"). Sistema conhecido pela pasta ⇒ exige bater; desconhecido ⇒
    // aceita se todos os candidatos forem do mesmo sistema.
    let match_roms = |system: &Option<String>, stem: &str| -> Vec<String> {
        let Some(cands) = by_stem.get(&stem.to_lowercase()) else {
            return Vec::new();
        };
        match system {
            Some(sys) => cands
                .iter()
                .filter(|(s, _)| s == sys)
                .map(|(_, id)| id.clone())
                .collect(),
            None => {
                let same_sys = cands.iter().all(|(s, _)| s == &cands[0].0);
                if same_sys {
                    cands.iter().map(|(_, id)| id.clone()).collect()
                } else {
                    Vec::new() // ambíguo (mesmo nome em sistemas diferentes)
                }
            }
        }
    };

    let pack_id = format!("pack:{}", path.display());
    let pack_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("bezels")
        .to_string();

    let mut assignments = Vec::new();
    let mut game_bezels_matched = 0usize;
    for d in &scanned {
        let asset_path = d.asset_path.display().to_string();
        match &d.scope {
            DecoScope::Default => assignments.push(DecorationAssignment {
                scope: AssignmentScope::Default,
                system_id: None,
                rom_id: None,
                pack_id: pack_id.clone(),
                asset_path,
            }),
            DecoScope::System(sys) => assignments.push(DecorationAssignment {
                scope: AssignmentScope::System,
                system_id: Some(sys.clone()),
                rom_id: None,
                pack_id: pack_id.clone(),
                asset_path,
            }),
            DecoScope::Rom { system, stem } => {
                let ids = match_roms(system, stem);
                if ids.is_empty() {
                    continue; // nenhuma ROM correspondente na biblioteca
                }
                game_bezels_matched += 1;
                for rom_id in ids {
                    assignments.push(DecorationAssignment {
                        scope: AssignmentScope::Rom,
                        system_id: None,
                        rom_id: Some(rom_id),
                        pack_id: pack_id.clone(),
                        asset_path: asset_path.clone(),
                    });
                }
            }
        }
    }

    // Packs grandes cobrem o mesmo jogo em várias pastas — só a 1ª atribuição
    // de cada alvo conta (os índices únicos por escopo exigem isso).
    let mut seen = std::collections::HashSet::new();
    assignments.retain(|a| {
        let key = match a.scope {
            AssignmentScope::Default => "default".to_string(),
            AssignmentScope::System => format!("s:{}", a.system_id.as_deref().unwrap_or("")),
            AssignmentScope::Rom => format!("r:{}", a.rom_id.as_deref().unwrap_or("")),
        };
        seen.insert(key)
    });

    let (n_def, n_sys, n_rom) = assignments
        .iter()
        .fold((0, 0, 0), |(d, s, r), a| match a.scope {
            AssignmentScope::Default => (d + 1, s, r),
            AssignmentScope::System => (d, s + 1, r),
            AssignmentScope::Rom => (d, s, r + 1),
        });
    log::info!(
        "decoração: {} atribuição(ões) — {n_def} default, {n_sys} sistema, {n_rom} jogo \
         ({game_bezels_matched} bezel(s) de jogo casaram, duplicatas da biblioteca incluídas)",
        assignments.len()
    );
    if assignments.is_empty() {
        return Err(format!(
            "{} imagem(ns) encontrada(s), mas nenhuma casou com um jogo/sistema da biblioteca \
             (nomes de arquivo têm que bater com as ROMs)",
            scanned.len()
        ));
    }

    let sc = db::DecorationRepo::new(pool.clone());
    sc.upsert_pack(&DecorationPack {
        id: pack_id.clone(),
        name: pack_name,
        source: DecorationPackSource::UserImported,
        base_path: path.display().to_string(),
    })
    .await
    .map_err(|e| e.to_string())?;
    sc.replace_assignments(&pack_id, &assignments)
        .await
        .map_err(|e| e.to_string())?;

    let persisted = sc.count_assignments().await.unwrap_or(-1);
    log::info!("decoração: {persisted} atribuição(ões) no banco após import");

    Ok(assignments.len())
}

/// Decodifica um PNG qualquer → `(rgba8, w, h)`. Os bezels do Bezel Project /
/// RetroBat são **PNG paletado (colortype 3) com tRNS** — por isso a gente pede
/// `EXPAND` (paleta→RGB, tRNS→alpha, <8bit→8bit) + `STRIP_16` (16→8bit) e
/// depois normaliza Gray/GrayAlpha/RGB/RGBA pra RGBA8.
pub fn decode_png(path: &Path) -> Result<(Vec<u8>, u32, u32), String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err(format!(
            "PNG {:?} não suportado após EXPAND",
            info.bit_depth
        ));
    }
    let px = (info.width * info.height) as usize;
    let mut rgba = vec![0u8; px * 4];
    match info.color_type {
        png::ColorType::Rgba => rgba.copy_from_slice(&buf[..px * 4]),
        png::ColorType::Rgb => {
            for i in 0..px {
                rgba[i * 4..i * 4 + 3].copy_from_slice(&buf[i * 3..i * 3 + 3]);
                rgba[i * 4 + 3] = 255;
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for i in 0..px {
                let (g, a) = (buf[i * 2], buf[i * 2 + 1]);
                rgba[i * 4..i * 4 + 3].copy_from_slice(&[g, g, g]);
                rgba[i * 4 + 3] = a;
            }
        }
        png::ColorType::Grayscale => {
            for i in 0..px {
                let g = buf[i];
                rgba[i * 4..i * 4 + 4].copy_from_slice(&[g, g, g, 255]);
            }
        }
        png::ColorType::Indexed => {
            return Err("PNG ainda paletado após EXPAND (inesperado)".into());
        }
    }
    Ok((rgba, info.width, info.height))
}

#[cfg(test)]
mod tests {
    use super::decode_png;
    use std::io::BufWriter;

    /// Os bezels do Bezel Project são PNG paletado (colortype 3) + tRNS —
    /// `decode_png` tem que expandir pra RGBA8 em vez de recusar.
    #[test]
    fn decodes_indexed_png_with_trns() {
        let dir = std::env::temp_dir().join("reemu_png_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("indexed.png");

        let file = std::fs::File::create(&path).unwrap();
        let mut enc = png::Encoder::new(BufWriter::new(file), 2, 2);
        enc.set_color(png::ColorType::Indexed);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_palette(vec![255, 0, 0, 0, 255, 0]); // idx0 vermelho, idx1 verde
        enc.set_trns(vec![0u8, 255u8]); // idx0 transparente, idx1 opaco
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&[0, 1, 1, 0]).unwrap();
        w.finish().unwrap();

        let (rgba, width, height) = decode_png(&path).unwrap();
        assert_eq!((width, height), (2, 2));
        assert_eq!(rgba.len(), 16);
        assert_eq!(&rgba[0..4], &[255, 0, 0, 0]); // vermelho transparente
        assert_eq!(&rgba[4..8], &[0, 255, 0, 255]); // verde opaco
    }
}
