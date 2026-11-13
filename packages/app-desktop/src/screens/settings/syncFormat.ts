import type { HardwarePayload } from '../../lib/tauri'

/** Oito matizes bem separados, todos escuros o bastante para o texto branco. */
export const MACHINE_COLORS = [
  'hsl(262 45% 42%)',
  'hsl(205 60% 34%)',
  'hsl(165 55% 28%)',
  'hsl(95 45% 30%)',
  'hsl(38 70% 34%)',
  'hsl(12 60% 40%)',
  'hsl(335 50% 40%)',
  'hsl(230 45% 45%)',
] as const

/** Cor estável do avatar de uma máquina: o hash do id escolhe da paleta. */
export function machineColor(id: string): string {
  let h = 0
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) >>> 0
  return MACHINE_COLORS[h % MACHINE_COLORS.length]
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
