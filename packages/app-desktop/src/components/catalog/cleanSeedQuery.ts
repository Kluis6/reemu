const INNER_ROM_EXT_RE =
  /\.(?:nkit|wbfs|iso|ciso|cso|chd|rvz|gcz|wia|gcm|mdf|mds|img|bin|cue|ccd|sub|ecm|pbp|xiso|nsp|xci|cia|3ds|nds|dsi|gba|gbc|gb|sfc|smc|nes|fds|unf|z64|n64|v64|md|gen|smd|32x|sms|gg|sg|pce|ngp|ngc|ws|wsc|lnx|j64|a26|a52|a78|col|int|vec|rom|dsk|adf|tzx|tap|z80|sna|prg|d64|t64|crt)$/i
const SONY_SERIAL_PREFIX_RE =
  /^(?:S[CL][A-Z]{2}|U[CL][A-Z]{2}|NP[A-Z]{2}|P[AB]PX)[\s._-]?\d{3}[._-]?\d{2}(?:[\s._:-]+|$)/i
const SEGA_SERIAL_PREFIX_RE = /^(?:MK-\d{4,5}|HDR-\d{4}|T-\d{4,6}[A-Z]?)\s*[-_:]\s*/i
const NINTENDO_DISC_ID_PREFIX_RE =
  /^[GDRPUW][A-Z0-9]{2}[EFJPKXDUW][0-9A-Z]{2}(?:\s*[-_:]\s*|\s+|\.)(?=[A-Za-z0-9]{2,})/
const SCENE_TAIL_RE =
  /(?:\s+[-_:]?\s*)(?:(?:PS2DVD|PS2CD|PSXPSP|PSPUMD|XBOXDVD|GCISO|WIIISO)(?:-[A-Z0-9_]+)?|NTSC-?[UJ]|MULTI\d+|PROPER|READNFO|REPACK|INTERNAL)(?:-[A-Z0-9_]+)?$/i
const DOT_SCENE_REGION_TAIL_RE = /\s+(?:USA|EUR|PAL|JPN|JAP|KOR|ntsc|pal)(?:-[A-Z0-9_]+)?$/i
const BARE_MEDIA_VER_RE =
  /(?:\s+[-:]\s*|\s+)(?:(?:disc|disk|cd|dvd|gd|umd)\s*\d+(?:\s*of\s*\d+)?|(?:side|track|chip)\s*(?:\d+|[a-d])\b|v\d+(?:\.\d+)*[a-z]?|ver\.?\s*\d+(?:\.\d+)*|rev\.?\s*(?:\d+|[a-c])\b)$/i
const INVERTED_ARTICLE_RE =
  /^(.{2,}?),\s*(the|a|an|el|la|los|las|le|les|der|die|das|o|os|as)$/i

function stripBracketedTags(input: string): string {
  let out = ''
  let paren = 0
  let bracket = 0
  let brace = 0
  for (const ch of input) {
    if (ch === '(') paren += 1
    else if (ch === ')') paren = Math.max(0, paren - 1)
    else if (ch === '[') bracket += 1
    else if (ch === ']') bracket = Math.max(0, bracket - 1)
    else if (ch === '{') brace += 1
    else if (ch === '}') brace = Math.max(0, brace - 1)
    else if (paren === 0 && bracket === 0 && brace === 0) out += ch
  }
  return out
}

function restoreInvertedArticle(segment: string): string {
  const trimmed = segment.trim()
  const m = INVERTED_ARTICLE_RE.exec(trimmed)
  return m ? `${m[2].trim()} ${m[1].trim()}` : trimmed
}

export function cleanSeedQuery(raw: string): string {
  let s = raw.trim()
  if (!s) return ''

  while (INNER_ROM_EXT_RE.test(s)) {
    const next = s.replace(INNER_ROM_EXT_RE, '')
    if (!next || next === s) break
    s = next
  }

  s = stripBracketedTags(s).trim()
  if (!s) return ''

  s = s
    .replace(SONY_SERIAL_PREFIX_RE, '')
    .replace(SEGA_SERIAL_PREFIX_RE, '')
    .replace(NINTENDO_DISC_ID_PREFIX_RE, '')
    .replace(/_/g, ' ')

  const wasDotScene = !s.includes(' ') && (s.match(/\./g)?.length ?? 0) >= 2
  if (wasDotScene) {
    s = s.replace(/\./g, ' ')
  }
  s = s.replace(/\s+/g, ' ').trim()

  s = s.replace(/^#\d{1,5}\s*(?:[-._:]\s*|\s+)/, '')
  const zeroPad = /^(0\d{1,4})\s*(?:[-._:]\s*|\s+)(.+)$/.exec(s)
  if (zeroPad && zeroPad[1] !== '007') {
    s = zeroPad[2]
  } else {
    const fourDigit = /^([1-9]\d{3})\s*-\s+([A-Za-z].*)$/.exec(s)
    if (fourDigit) {
      const n = Number(fourDigit[1])
      if ((n < 1900 || n > 2099) && n !== 1080) {
        s = fourDigit[2]
      }
    }
  }

  for (;;) {
    const prev = s
    s = s.replace(SCENE_TAIL_RE, '').trim()
    if (wasDotScene) {
      s = s.replace(DOT_SCENE_REGION_TAIL_RE, '').trim()
    }
    s = s.replace(BARE_MEDIA_VER_RE, '').trim()
    if (s === prev) break
  }

  s = s.replace(/^[-:;, ]+|[-:;, ]+$/g, '').replace(/\s+/g, ' ').trim()
  if (!s) return ''

  s = s.includes(' - ')
    ? s.split(' - ').map(restoreInvertedArticle).join(' - ')
    : restoreInvertedArticle(s)

  return s.replace(/\s+/g, ' ').trim()
}
