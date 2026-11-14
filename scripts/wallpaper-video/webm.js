// Gravador mínimo de WebM (Matroska) para um fluxo VP9 vindo do `VideoEncoder`
// (WebCodecs). Só o necessário para tocar em loop no WebView: EBML header,
// Segment com SeekHead, Info (duração), Tracks, Clusters de SimpleBlock e Cues.
// Referência: especificação Matroska/WebM (matroska.org, "Matroska Elements";
// webmproject.org, "WebM Container Guidelines"). Tamanhos sempre em vint de
// 8 bytes: válido no EBML e deixa o SeekHead com tamanho fixo.

const ID = {
  EBML: [0x1a, 0x45, 0xdf, 0xa3], EBMLVersion: [0x42, 0x86], EBMLReadVersion: [0x42, 0xf7],
  EBMLMaxIDLength: [0x42, 0xf2], EBMLMaxSizeLength: [0x42, 0xf3], DocType: [0x42, 0x82],
  DocTypeVersion: [0x42, 0x87], DocTypeReadVersion: [0x42, 0x85],
  Segment: [0x18, 0x53, 0x80, 0x67], SeekHead: [0x11, 0x4d, 0x9b, 0x74], Seek: [0x4d, 0xbb],
  SeekID: [0x53, 0xab], SeekPosition: [0x53, 0xac],
  Info: [0x15, 0x49, 0xa9, 0x66], TimecodeScale: [0x2a, 0xd7, 0xb1], Duration: [0x44, 0x89],
  MuxingApp: [0x4d, 0x80], WritingApp: [0x57, 0x41],
  Tracks: [0x16, 0x54, 0xae, 0x6b], TrackEntry: [0xae], TrackNumber: [0xd7], TrackUID: [0x73, 0xc5],
  TrackType: [0x83], FlagLacing: [0x9c], CodecID: [0x86], CodecPrivate: [0x63, 0xa2], Video: [0xe0], PixelWidth: [0xb0],
  PixelHeight: [0xba],
  Cluster: [0x1f, 0x43, 0xb6, 0x75], Timecode: [0xe7], SimpleBlock: [0xa3],
  Cues: [0x1c, 0x53, 0xbb, 0x6b], CuePoint: [0xbb], CueTime: [0xb3], CueTrackPositions: [0xb7],
  CueTrack: [0xf7], CueClusterPosition: [0xf1],
}

function size8(n) {
  const b = new Uint8Array(8)
  b[0] = 0x01
  let v = n
  for (let i = 7; i >= 1; i--) { b[i] = v % 256; v = Math.floor(v / 256) }
  return b
}
function uint(n, bytes) {
  const len = bytes ?? Math.max(1, Math.ceil(Math.log2(n + 1) / 8))
  const b = new Uint8Array(len)
  let v = n
  for (let i = len - 1; i >= 0; i--) { b[i] = v % 256; v = Math.floor(v / 256) }
  return b
}
function f64(x) { const b = new Uint8Array(8); new DataView(b.buffer).setFloat64(0, x); return b }
function str(s) { return new TextEncoder().encode(s) }
function cat(parts) {
  const len = parts.reduce((a, p) => a + p.length, 0)
  const out = new Uint8Array(len)
  let o = 0
  for (const p of parts) { out.set(p, o); o += p.length }
  return out
}
function el(id, body) {
  const data = body instanceof Uint8Array ? body : cat(body)
  return cat([Uint8Array.from(id), size8(data.length), data])
}

/** frames: [{ data: Uint8Array, timeMs: number, key: boolean }]; `codecId`
 *  `V_VP9` ou `V_AV1` (este exige `codecPrivate`, o `av1C` que o encoder dá em
 *  `decoderConfig.description`). */
export function muxWebm({ width, height, frames, durationMs, codecId = 'V_VP9', codecPrivate = null }) {
  const header = el(ID.EBML, [
    el(ID.EBMLVersion, uint(1)), el(ID.EBMLReadVersion, uint(1)),
    el(ID.EBMLMaxIDLength, uint(4)), el(ID.EBMLMaxSizeLength, uint(8)),
    el(ID.DocType, str('webm')), el(ID.DocTypeVersion, uint(4)), el(ID.DocTypeReadVersion, uint(2)),
  ])
  const info = el(ID.Info, [
    el(ID.TimecodeScale, uint(1000000)), el(ID.Duration, f64(durationMs)),
    el(ID.MuxingApp, str('reemu-wallpaper')), el(ID.WritingApp, str('reemu-wallpaper')),
  ])
  const tracks = el(ID.Tracks, [el(ID.TrackEntry, [
    el(ID.TrackNumber, uint(1)), el(ID.TrackUID, uint(1)), el(ID.TrackType, uint(1)),
    el(ID.FlagLacing, uint(0)), el(ID.CodecID, str(codecId)),
    ...(codecPrivate ? [el(ID.CodecPrivate, codecPrivate)] : []),
    el(ID.Video, [el(ID.PixelWidth, uint(width)), el(ID.PixelHeight, uint(height))]),
  ])])

  // um Cluster por quadro-chave
  const clusters = []
  let cur = null
  for (const f of frames) {
    if (f.key || !cur) { cur = { time: f.timeMs, blocks: [] }; clusters.push(cur) }
    const rel = f.timeMs - cur.time
    const head = new Uint8Array(4)
    head[0] = 0x81 // track 1
    new DataView(head.buffer).setInt16(1, rel)
    head[3] = f.key ? 0x80 : 0x00
    cur.blocks.push(el(ID.SimpleBlock, [head, f.data]))
  }
  const clusterBytes = clusters.map((c) => el(ID.Cluster, [el(ID.Timecode, uint(c.time, 8)), ...c.blocks]))

  // SeekHead de tamanho fixo (posições em 8 bytes): calcula com um falso
  const seekEntry = (id, pos) => el(ID.Seek, [el(ID.SeekID, Uint8Array.from(id)), el(ID.SeekPosition, uint(pos, 8))])
  const seekLen = el(ID.SeekHead, [seekEntry(ID.Info, 0), seekEntry(ID.Tracks, 0), seekEntry(ID.Cues, 0)]).length
  const infoPos = seekLen
  const tracksPos = infoPos + info.length
  let pos = tracksPos + tracks.length
  const cuePoints = []
  for (let i = 0; i < clusters.length; i++) {
    cuePoints.push(el(ID.CuePoint, [
      el(ID.CueTime, uint(clusters[i].time, 8)),
      el(ID.CueTrackPositions, [el(ID.CueTrack, uint(1)), el(ID.CueClusterPosition, uint(pos, 8))]),
    ]))
    pos += clusterBytes[i].length
  }
  const cues = el(ID.Cues, cuePoints)
  const cuesPos = pos
  const seekHead = el(ID.SeekHead, [seekEntry(ID.Info, infoPos), seekEntry(ID.Tracks, tracksPos), seekEntry(ID.Cues, cuesPos)])
  const segment = el(ID.Segment, [seekHead, info, tracks, ...clusterBytes, cues])
  return cat([header, segment])
}
