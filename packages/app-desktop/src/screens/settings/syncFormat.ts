import type { HardwarePayload } from '../../lib/tauri'

/** Cor estável do avatar de uma máquina: o matiz sai do hash do id. */
export function machineColor(id: string): string {
  let h = 0
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) >>> 0
  return `hsl(${h % 360} 45% 38%)`
}

/** Ex.: "Windows 11 · Ryzen 7 5800X · 32 GB · RTX 3070 · Vulkan". Campo vazio some. */
export function hardwareLine(hw: HardwarePayload | null, lang: string): string {
  if (!hw) return ''
  const memory =
    hw.memory_mb > 0
      ? `${new Intl.NumberFormat(lang, { maximumFractionDigits: 1 }).format(hw.memory_mb / 1024)} GB`
      : ''
  return [hw.os, hw.cpu, memory, hw.gpu, hw.gpu_backend].filter((p) => p.trim() !== '').join(' · ')
}

export function formatBytes(bytes: number, lang: string): string {
  const n = (v: number) => new Intl.NumberFormat(lang, { maximumFractionDigits: 1 }).format(v)
  if (bytes >= 1024 ** 3) return `${n(bytes / 1024 ** 3)} GB`
  if (bytes >= 1024 ** 2) return `${n(bytes / 1024 ** 2)} MB`
  return `${n(bytes / 1024)} KB`
}
