//! Atualização automática do app (`tauri-plugin-updater`).
//!
//! O CI (`release.yml`) assina os instaladores e publica um `latest.json` na
//! Release do GitHub; o endpoint em `tauri.conf.json` aponta pra
//! `releases/latest/download/latest.json` — então só Releases **publicadas**
//! (não draft, não pre-release) chegam aos usuários.
//!
//! Fluxo: `update_check` guarda o `Update` encontrado aqui e devolve o resumo
//! pro frontend (toast + sino + modal); `update_install` baixa (emitindo
//! `update-progress`), verifica a assinatura, instala e reinicia.
//!
//! Sem chave pública em `plugins.updater.pubkey` a verificação fica desligada
//! (`update_check` devolve `None`) — o app não quebra antes de a chave de
//! assinatura existir.

use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Default)]
pub struct UpdateState(Mutex<Option<Update>>);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    /// Notas da Release (Markdown do corpo da Release no GitHub).
    pub notes: Option<String>,
    /// RFC 3339.
    pub date: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

fn pubkey_configured(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .is_some_and(|k| !k.trim().is_empty())
}

/// Só em build de debug: `REEMU_FAKE_UPDATE=1` finge que há versão nova —
/// pra ver toast/sino/modal sem publicar Release.
fn fake_update() -> bool {
    cfg!(debug_assertions) && std::env::var_os("REEMU_FAKE_UPDATE").is_some_and(|v| v == "1")
}

#[tauri::command]
pub async fn update_check(
    app: AppHandle,
    state: State<'_, UpdateState>,
) -> Result<Option<UpdateInfo>, String> {
    if fake_update() {
        return Ok(Some(UpdateInfo {
            version: "9.9.9".into(),
            current_version: app.package_info().version.to_string(),
            notes: Some(
                "## Novidades\n\n- Canvas de vídeo com WebGL\n- Sino de notificações na barra lateral\n\n## Correções\n\n- Exemplo de nota (REEMU_FAKE_UPDATE)"
                    .into(),
            ),
            date: None,
        }));
    }
    if !pubkey_configured(&app) {
        log::info!("atualização: sem chave pública no tauri.conf.json — verificação desligada");
        return Ok(None);
    }
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        notes: u.body.clone(),
        date: u
            .raw_json
            .get("pub_date")
            .and_then(|d| d.as_str())
            .map(str::to_owned),
    });
    *state.0.lock().unwrap_or_else(|p| p.into_inner()) = update;
    Ok(info)
}

/// Baixa, verifica a assinatura e instala a versão achada no último
/// `update_check`; depois reinicia o app. No Windows o instalador (NSIS/MSI)
/// fecha o processo sozinho; no Linux o AppImage é trocado no lugar (ou o
/// `.deb` instalado via `pkexec`).
#[tauri::command]
pub async fn update_install(app: AppHandle, state: State<'_, UpdateState>) -> Result<(), String> {
    if fake_update() {
        let total = 40_000_000u64;
        for i in 1..=20u64 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = app.emit(
                "update-progress",
                UpdateProgress {
                    downloaded: total * i / 20,
                    total: Some(total),
                },
            );
        }
        return Err("atualização simulada (REEMU_FAKE_UPDATE) — nada foi instalado".into());
    }
    let update = state
        .0
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .ok_or("nenhuma atualização pendente — verifique de novo")?;

    // Descarrega o jogo antes (flush da save RAM), como no fechamento normal.
    let _ = app.state::<crate::commands::AppState>().session.unload();

    let mut downloaded = 0u64;
    let progress_app = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress_app.emit("update-progress", UpdateProgress { downloaded, total });
            },
            || log::info!("atualização: download concluído, instalando"),
        )
        .await
        .map_err(|e| e.to_string())?;
    log::info!("atualização: instalada {} — reiniciando", update.version);
    app.restart();
}

#[cfg(test)]
mod tests {
    /// O plugin desserializa `plugins.updater` no setup — config inválida
    /// derruba o app ao abrir. Garante que a do `tauri.conf.json` passa.
    #[test]
    fn tauri_conf_updater_config_parses() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let updater = conf["plugins"]["updater"].clone();
        let cfg: tauri_plugin_updater::Config = serde_json::from_value(updater).unwrap();
        assert_eq!(cfg.endpoints.len(), 1);
        assert!(cfg.require_signed_version);
    }
}
