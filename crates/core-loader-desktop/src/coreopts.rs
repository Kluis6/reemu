//! Core options (`RETRO_ENVIRONMENT_SET_VARIABLES` v0 / `SET_CORE_OPTIONS`
//! v1 / `SET_CORE_OPTIONS_V2`, com as variantes `_INTL`).
//!
//! O core declara as opções durante `retro_set_environment` / `retro_load_game`;
//! guardamos o schema + os valores atuais no `FrontendState` global. O core
//! lê o valor de volta via `GET_VARIABLE` e checa mudança via
//! `GET_VARIABLE_UPDATE` (usado quando o usuário troca uma opção em runtime).
//!
//! Categorias (v2) são ignoradas — a UI é uma lista plana gerada do schema.

use crate::ffi_state;
use crate::sys;
use domain::core_options::{CoreOptionDefinition, CoreOptionType};
use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CoreOption {
    pub key: String,
    pub desc: String,
    pub values: Vec<String>,
    pub default: String,
}

/// Padrões do ReEmu que substituem o padrão declarado pelo core (`key`,
/// `valor`). Só valem se o core oferecer esse valor; a escolha do usuário
/// (vinda do banco) continua tendo prioridade.
///
/// - `reicast_detect_vsync_swap_interval` (flycast, "Detect Frame Rate
///   Changes", `disabled` no core): avisa por `SET_SYSTEM_AV_INFO` quando o
///   jogo cai pra 30/20 fps. Sem ela, com Threaded Rendering, cada
///   `retro_run` emula 2 refreshes num jogo a 30 fps e o áudio sai em dobro
///   (Jet Set Radio, 2026-10-02). A descrição no core
///   (`shell/libretro/libretro_core_options.h`) pede desligado em jogos de
///   taxa instável (Ecco, Unreal Tournament); aí o usuário desliga por jogo.
const FRONTEND_DEFAULTS: &[(&str, &str)] = &[("reicast_detect_vsync_swap_interval", "enabled")];

/// Aplica `FRONTEND_DEFAULTS` ao schema declarado pelo core.
pub(crate) fn apply_frontend_defaults(opts: &mut [CoreOption]) {
    for o in opts.iter_mut() {
        if let Some((_, v)) = FRONTEND_DEFAULTS.iter().find(|(k, _)| *k == o.key) {
            if o.values.iter().any(|x| x == v) {
                o.default = (*v).to_string();
            }
        }
    }
}

pub(crate) unsafe fn cstr(p: *const c_char) -> Option<String> {
    (!p.is_null()).then(|| CStr::from_ptr(p).to_string_lossy().into_owned())
}

/// v0: `retro_variable[]` terminado por `key == null`. `value` = `"Desc; a|b|c"`.
pub(crate) unsafe fn parse_variables(mut p: *const sys::retro_variable) -> Vec<CoreOption> {
    let mut out = Vec::new();
    if p.is_null() {
        return out;
    }
    loop {
        let v = &*p;
        let (Some(key), Some(spec)) = (cstr(v.key), cstr(v.value)) else {
            break;
        };
        let (desc, rest) = spec.split_once(';').unwrap_or((spec.as_str(), ""));
        let values: Vec<String> = rest
            .split('|')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
        if let Some(default) = values.first().cloned() {
            out.push(CoreOption {
                key,
                desc: desc.trim().to_string(),
                values,
                default,
            });
        }
        p = p.add(1);
    }
    out
}

unsafe fn values_of(
    vals: &[sys::retro_core_option_value; sys::RETRO_NUM_CORE_OPTION_VALUES_MAX],
) -> Vec<String> {
    let mut out = Vec::new();
    for v in vals {
        match cstr(v.value) {
            Some(x) => out.push(x),
            None => break,
        }
    }
    out
}

fn pick_default(declared: Option<String>, values: &[String]) -> String {
    declared
        .filter(|d| values.contains(d))
        .or_else(|| values.first().cloned())
        .unwrap_or_default()
}

/// v1: `retro_core_option_definition[]` terminado por `key == null`.
pub(crate) unsafe fn parse_v1(mut p: *const sys::retro_core_option_definition) -> Vec<CoreOption> {
    let mut out = Vec::new();
    if p.is_null() {
        return out;
    }
    loop {
        let d = &*p;
        let Some(key) = cstr(d.key) else { break };
        let values = values_of(&d.values);
        if !values.is_empty() {
            let default = pick_default(cstr(d.default_value), &values);
            out.push(CoreOption {
                desc: cstr(d.desc)
                    .unwrap_or_else(|| key.clone())
                    .trim()
                    .to_string(),
                key,
                values,
                default,
            });
        }
        p = p.add(1);
    }
    out
}

/// v2: idem, com campos extras (categorias) que ignoramos.
pub(crate) unsafe fn parse_v2(
    mut p: *const sys::retro_core_option_v2_definition,
) -> Vec<CoreOption> {
    let mut out = Vec::new();
    if p.is_null() {
        return out;
    }
    loop {
        let d = &*p;
        let Some(key) = cstr(d.key) else { break };
        let values = values_of(&d.values);
        if !values.is_empty() {
            let default = pick_default(cstr(d.default_value), &values);
            out.push(CoreOption {
                desc: cstr(d.desc)
                    .unwrap_or_else(|| key.clone())
                    .trim()
                    .to_string(),
                key,
                values,
                default,
            });
        }
        p = p.add(1);
    }
    out
}

impl CoreOption {
    fn to_definition(&self) -> CoreOptionDefinition {
        CoreOptionDefinition {
            option_key: self.key.clone(),
            display_name: self.desc.clone(),
            option_type: CoreOptionType::Combo {
                choices: self.values.clone(),
            },
            default_value: self.default.clone(),
        }
    }
}

// --- valores pendentes (do DB) a aplicar no próximo core ---------------------

static PENDING: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// Valores salvos que o `FrontendState` do próximo core deve adotar. O app
/// chama isto antes de `EmuSession::load`.
pub fn set_pending_core_option_values(values: HashMap<String, String>) {
    *PENDING.lock().unwrap_or_else(|p| p.into_inner()) = Some(values);
}

pub(crate) fn take_pending_core_option_values() -> HashMap<String, String> {
    PENDING
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take()
        .unwrap_or_default()
}

// --- API pública lida/escrita de qualquer thread (Mutex global) -------------

/// Schema declarado pelo core carregado agora. Vazio se nenhum core, ou se o
/// core não declara opções.
pub fn core_options() -> Vec<CoreOptionDefinition> {
    ffi_state::lock()
        .as_ref()
        .map(|st| {
            st.core_options
                .iter()
                .map(CoreOption::to_definition)
                .collect()
        })
        .unwrap_or_default()
}

/// Valores atuais (`key -> value`) do core carregado agora.
pub fn core_option_values() -> HashMap<String, String> {
    ffi_state::lock()
        .as_ref()
        .map(|st| st.option_values.clone())
        .unwrap_or_default()
}

/// Troca uma opção do core em runtime. `false` se não há core, a chave não
/// existe, ou o valor não é uma das escolhas. O core percebe no próximo
/// `retro_run` (via `GET_VARIABLE_UPDATE`).
pub fn set_core_option(key: &str, value: &str) -> bool {
    ffi_state::lock()
        .as_mut()
        .map(|st| st.set_option_value(key, value))
        .unwrap_or(false)
}

/// Opções que o core trocou sozinho (`SET_VARIABLE`) desde a última
/// chamada — o frontend guarda, pra valerem nas próximas sessões.
pub fn take_core_set_options() -> Vec<(String, String)> {
    ffi_state::lock()
        .as_mut()
        .map(|st| std::mem::take(&mut st.core_set_options))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opt(key: &str, values: &[&str], default: &str) -> CoreOption {
        CoreOption {
            key: key.into(),
            desc: key.into(),
            values: values.iter().map(|v| v.to_string()).collect(),
            default: default.into(),
        }
    }

    #[test]
    fn frontend_default_overrides_the_core_default() {
        let mut opts = vec![
            opt(
                "reicast_detect_vsync_swap_interval",
                &["disabled", "enabled"],
                "disabled",
            ),
            opt("reicast_other", &["disabled", "enabled"], "disabled"),
        ];
        apply_frontend_defaults(&mut opts);
        assert_eq!(opts[0].default, "enabled");
        assert_eq!(opts[1].default, "disabled");
    }

    #[test]
    fn frontend_default_needs_a_value_the_core_offers() {
        let mut opts = vec![opt(
            "reicast_detect_vsync_swap_interval",
            &["off", "on"],
            "off",
        )];
        apply_frontend_defaults(&mut opts);
        assert_eq!(opts[0].default, "off");
    }
}
