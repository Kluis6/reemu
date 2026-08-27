//! Mapeamento explícito entre os enums do `domain` e a representação
//! textual no banco. Feito à mão de propósito: a serialização serde dos
//! enums (`"OpenGl"`, `"Combo"`, ...) não bate com os `CHECK` do schema
//! (`'opengl'`, `'combo'`, ...), e não queremos acoplar o schema à derive.

use domain::core_loader::RenderBackend;
use domain::core_options::CoreOptionType;
use domain::error::RepoError;
use domain::shader_chain::AssignmentScope;

// Usado pelos métodos de escrita de atribuição (criar/editar assignment),
// que entram junto da UI de shader/decoração — ainda não nesta etapa.
#[allow(dead_code)]
pub fn scope_to_db(scope: AssignmentScope) -> &'static str {
    match scope {
        AssignmentScope::Default => "default",
        AssignmentScope::System => "system",
        AssignmentScope::Rom => "rom",
    }
}

pub fn scope_from_db(s: &str) -> Result<AssignmentScope, RepoError> {
    match s {
        "default" => Ok(AssignmentScope::Default),
        "system" => Ok(AssignmentScope::System),
        "rom" => Ok(AssignmentScope::Rom),
        other => Err(RepoError::Corrupt(format!("scope desconhecido: {other:?}"))),
    }
}

pub fn render_backend_to_db(b: &RenderBackend) -> &'static str {
    match b {
        RenderBackend::Software => "software",
        RenderBackend::OpenGl => "opengl",
        RenderBackend::Vulkan => "vulkan",
    }
}

pub fn render_backend_from_db(s: &str) -> Result<RenderBackend, RepoError> {
    match s {
        "software" => Ok(RenderBackend::Software),
        "opengl" => Ok(RenderBackend::OpenGl),
        "vulkan" => Ok(RenderBackend::Vulkan),
        other => Err(RepoError::Corrupt(format!(
            "render_backend desconhecido: {other:?}"
        ))),
    }
}

/// `option_type` é uma string curta no banco (`CHECK IN ('combo','bool','range')`);
/// os parâmetros (`choices`, `min/max/step`) ficam na coluna `choices` como JSON.
pub fn option_type_to_db(t: &CoreOptionType) -> (&'static str, Option<String>) {
    match t {
        CoreOptionType::Combo { choices } => (
            "combo",
            Some(serde_json::json!({ "choices": choices }).to_string()),
        ),
        CoreOptionType::Bool => ("bool", None),
        CoreOptionType::Range { min, max, step } => (
            "range",
            Some(serde_json::json!({ "min": min, "max": max, "step": step }).to_string()),
        ),
    }
}

pub fn option_type_from_db(
    kind: &str,
    choices_json: Option<&str>,
) -> Result<CoreOptionType, RepoError> {
    let corrupt = |m: String| RepoError::Corrupt(m);
    match kind {
        "bool" => Ok(CoreOptionType::Bool),
        "combo" => {
            let raw =
                choices_json.ok_or_else(|| corrupt("option_type 'combo' sem choices".into()))?;
            let v: serde_json::Value = serde_json::from_str(raw)
                .map_err(|e| corrupt(format!("choices JSON inválido: {e}")))?;
            let choices = v
                .get("choices")
                .and_then(|c| c.as_array())
                .ok_or_else(|| corrupt("choices JSON sem array 'choices'".into()))?
                .iter()
                .map(|s| s.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| corrupt("choices com item não-string".into()))?;
            Ok(CoreOptionType::Combo { choices })
        }
        "range" => {
            let raw =
                choices_json.ok_or_else(|| corrupt("option_type 'range' sem parâmetros".into()))?;
            let v: serde_json::Value = serde_json::from_str(raw)
                .map_err(|e| corrupt(format!("range JSON inválido: {e}")))?;
            let num = |k: &str| {
                v.get(k)
                    .and_then(|x| x.as_f64())
                    .ok_or_else(|| corrupt(format!("range JSON sem '{k}'")))
            };
            Ok(CoreOptionType::Range {
                min: num("min")?,
                max: num("max")?,
                step: num("step")?,
            })
        }
        other => Err(corrupt(format!("option_type desconhecido: {other:?}"))),
    }
}
