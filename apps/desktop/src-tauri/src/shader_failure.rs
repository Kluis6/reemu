//! Shader que impede o jogo de aparecer (pedido do usuário, 2026-10-29): sai
//! da atribuição (jogo, plataforma ou padrão) e vira aviso no sino da barra
//! lateral (`shader-failed`), que abre um modal com o erro.
//!
//! Vem de três lugares: o preset não monta ao abrir o jogo, está na lista dos
//! que travam a GPU (`gpu::known_issues`), ou dá erro do wgpu no meio do jogo
//! (`FrameProcessor::recover_from_gpu_error`, que já voltou pro `plain`). Este
//! último roda dentro do processador, sem acesso ao app — por isso o
//! `AppHandle` fica guardado aqui desde o startup.

use std::sync::OnceLock;

use domain::shader_chain::{AssignmentScope, ShaderChainStore};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::AppState;

static APP: OnceLock<AppHandle> = OnceLock::new();

/// Chamado uma vez no `setup`.
pub fn init(app: AppHandle) {
    let _ = APP.set(app);
}

/// De onde veio o shader que está rodando no jogo aberto — pra saber qual
/// atribuição remover quando ele falha.
#[derive(Clone)]
pub struct ActiveShader {
    pub source: String,
    pub scope: AssignmentScope,
    pub system_id: Option<String>,
    pub rom_id: Option<String>,
}

/// O que deu errado (a tela traduz o tipo).
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FailureKind {
    /// O preset não montou (compilação/validação dos pipelines).
    Build,
    /// Na lista dos que travam a GPU: nem foi aplicado.
    KnownGpuHang,
    /// Erro do wgpu renderizando no meio do jogo.
    Gpu,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShaderFailedEvent {
    /// Caminho do `.slangp` (ou nome do builtin).
    source: String,
    /// Nome curto, pra mostrar.
    name: String,
    kind: FailureKind,
    /// `rom` | `system` | `default`; `None` = não estava atribuído (só nesta
    /// sessão).
    scope: Option<&'static str>,
    system_id: Option<String>,
    /// Mensagem técnica (do glslang/naga/wgpu), pro modal.
    error: String,
}

fn scope_key(s: AssignmentScope) -> &'static str {
    match s {
        AssignmentScope::Rom => "rom",
        AssignmentScope::System => "system",
        AssignmentScope::Default => "default",
    }
}

/// Remove a atribuição que trouxe `source` pro jogo aberto e avisa a tela.
/// Não trava: o trabalho com o banco vai numa task. Não pega o lock da GPU
/// (pode ser chamado de dentro do `FrameProcessor`).
pub fn report(source: &str, kind: FailureKind, error: String) {
    log::error!("shader '{source}' impediu o jogo de aparecer: {error} — removido da atribuição");
    let Some(app) = APP.get().cloned() else {
        return;
    };
    let source = source.to_string();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        // a atribuição de onde veio ESTE shader (`take_if` pede Rust 1.80; o
        // MSRV do crate é 1.77)
        let active = {
            let mut slot = state
                .active_shader
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            if slot.as_ref().is_some_and(|a| a.source == source) {
                slot.take()
            } else {
                None
            }
        };
        let (mut scope, mut system_id) = (None, None);
        if let (Some(a), Some(pool)) = (active, state.db.clone()) {
            let sc = db::ShaderChainRepo::new(pool);
            match sc
                .clear_assignment(a.scope, a.system_id.as_deref(), a.rom_id.as_deref())
                .await
            {
                Ok(()) => {
                    scope = Some(scope_key(a.scope));
                    system_id = a.system_id;
                }
                Err(e) => log::error!("shader '{source}': não deu pra remover a atribuição: {e}"),
            }
        }
        let name = std::path::Path::new(&source)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&source)
            .to_string();
        let _ = app.emit(
            "shader-failed",
            ShaderFailedEvent {
                source,
                name,
                kind,
                scope,
                system_id,
                error,
            },
        );
    });
}

/// A moldura saiu porque a imagem do jogo está larga (16:10 pra cima). Vai pro
/// sino: aviso nenhum aparece por cima do jogo.
pub fn report_wide_bezel() {
    log::info!("decoração: fora, a imagem do jogo está larga");
    if let Some(app) = APP.get() {
        let _ = app.emit("wide-bezel-off", ());
    }
}
