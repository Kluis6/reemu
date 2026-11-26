//! Configurações › Sistema: informações do hardware (só leitura).

use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "HardwareInfo"))]
pub struct HardwareInfo {
    /// Ex.: "Windows 11 Pro 26300".
    pub os: Option<String>,
    pub arch: String,
    pub cpu: Option<String>,
    pub cpu_cores: Option<usize>,
    pub cpu_threads: usize,
    /// Memória total em bytes.
    pub memory_bytes: u64,
    pub gpu: Option<String>,
    /// API do compositor (Vulkan, Dx12, …).
    pub gpu_backend: Option<String>,
    /// Driver e versão, como o wgpu informa.
    pub gpu_driver: Option<String>,
}

#[tauri::command]
pub fn get_hardware_info(state: State<'_, AppState>) -> HardwareInfo {
    use sysinfo::System;
    let sys = System::new().ok().map(|mut sys| {
        sys.refresh_cpu_all();
        sys.refresh_memory();
        sys
    });
    let cpu = sys
        .as_ref()
        .and_then(|sys| sys.cpus().first())
        .map(|c| c.brand().trim().to_string())
        .filter(|b| !b.is_empty());
    let gpu = state
        .gpu
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .map(|fp| fp.adapter_info());
    let driver = gpu.as_ref().and_then(|i| {
        let d = [i.driver.trim(), i.driver_info.trim()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        (!d.is_empty()).then_some(d)
    });
    HardwareInfo {
        os: System::long_os_version().ok(),
        arch: System::cpu_arch(),
        cpu,
        cpu_cores: System::physical_core_count().ok(),
        cpu_threads: sys.as_ref().map_or(0, |sys| sys.cpus().len()),
        memory_bytes: sys.as_ref().map_or(0, System::total_memory),
        gpu: gpu.as_ref().map(|i| i.name.clone()),
        gpu_backend: gpu.as_ref().map(|i| format!("{:?}", i.backend)),
        gpu_driver: driver,
    }
}
