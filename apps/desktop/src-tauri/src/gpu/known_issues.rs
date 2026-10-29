//! Presets do pacote `slang-shaders` com problema conhecido na GPU — a tela
//! avisa quando o usuário escolhe um deles ou abre um jogo com um deles
//! atribuído (pedido do usuário, 2026-10-29).
//!
//! A lista (`known_issues.txt`) sai do teste `field_render_all_presets`
//! (`gpu/tests.rs`), que roda cada preset de verdade na GPU: com
//! `REEMU_SHADER_ISSUES_OUT=<arquivo>` ele grava as linhas no formato daqui.
//! Medida numa RTX 3060 (Vulkan, Windows); em outra GPU o resultado pode
//! mudar — por isso é um aviso, não um bloqueio.

const LIST: &str = include_str!("known_issues.txt");

/// O que acontece com o preset na GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderIssue {
    /// A GPU trava e o driver é reiniciado (device perdido): a imagem do jogo
    /// para até reabrir o ReEmu.
    GpuHang,
    /// Erro do WebGPU ao renderizar: o ReEmu volta pro `plain` sozinho.
    RenderError,
}

impl ShaderIssue {
    /// Chave que a tela traduz (`video.shaderIssue.<chave>`).
    pub fn key(self) -> &'static str {
        match self {
            ShaderIssue::GpuHang => "gpuHang",
            ShaderIssue::RenderError => "renderError",
        }
    }
}

/// Problema conhecido do preset `name` (caminho de `.slangp`, ou
/// `curated:<id>`, que vira o caminho dele). `None` = nada conhecido.
pub fn shader_issue(name: &str) -> Option<ShaderIssue> {
    let path = match super::curated_by_wire(name) {
        Some(c) => super::curated_slangp(c)?.to_string_lossy().into_owned(),
        None => name.to_string(),
    };
    issue_for_path(&path)
}

/// A linha é `<tipo>\t<caminho relativo à raiz do pacote>`, com `/`.
fn issue_for_path(path: &str) -> Option<ShaderIssue> {
    let p = path.replace('\\', "/").to_ascii_lowercase();
    LIST.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .find_map(|l| {
            let (kind, rel) = l.split_once('\t')?;
            let rel = rel.trim().to_ascii_lowercase();
            let issue = match kind {
                "gpu-hang" => ShaderIssue::GpuHang,
                _ => ShaderIssue::RenderError,
            };
            (p == rel || p.ends_with(&format!("/{rel}"))).then_some(issue)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_by_relative_path_any_separator_and_case() {
        let line = LIST
            .lines()
            .find(|l| l.starts_with("gpu-hang\t"))
            .expect("a lista tem ao menos um gpu-hang");
        let rel = line.split_once('\t').unwrap().1;
        let full = format!(
            "C:\\Users\\x\\shaders\\slang-shaders\\{}",
            rel.replace('/', "\\")
        );
        assert_eq!(issue_for_path(&full), Some(ShaderIssue::GpuHang));
        assert_eq!(
            issue_for_path(&full.to_uppercase()),
            Some(ShaderIssue::GpuHang)
        );
        assert_eq!(issue_for_path("plain"), None);
        assert_eq!(issue_for_path("C:/x/crt/crt-guest-advanced.slangp"), None);
    }
}
