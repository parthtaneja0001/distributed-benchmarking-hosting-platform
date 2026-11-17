// Client-side port of `detect_language` in services/submission/src/events.rs.
// Unpacks a .tar.gz in the browser and applies the same rules so demo mode
// reports the same language the real backend would.

function readString(bytes, start, len) {
  const slice = bytes.subarray(start, start + len)
  const end = slice.indexOf(0)
  return new TextDecoder().decode(end === -1 ? slice : slice.subarray(0, end))
}

function listTarEntries(bytes) {
  const names = []
  let offset = 0
  let longName = null

  while (offset + 512 <= bytes.length) {
    const header = bytes.subarray(offset, offset + 512)
    if (header.every((b) => b === 0)) break // end-of-archive marker

    const size = parseInt(readString(header, 124, 12).trim() || '0', 8) || 0
    const type = String.fromCharCode(header[156])
    const dataStart = offset + 512

    if (type === 'L') {
      // GNU long-name record: the payload is the next entry's full path.
      longName = readString(bytes, dataStart, size)
    } else if (type !== 'x' && type !== 'g') {
      const prefix = readString(header, 345, 155)
      const name = readString(header, 0, 100)
      names.push(longName ?? (prefix ? `${prefix}/${name}` : name))
      longName = null
    }

    offset = dataStart + Math.ceil(size / 512) * 512
  }
  return names
}

/** Returns "go" | "rust" | "cpp" | "unknown", matching the backend. */
export async function detectLanguage(file) {
  try {
    if (typeof DecompressionStream === 'undefined') return 'unknown'
    const stream = file.stream().pipeThrough(new DecompressionStream('gzip'))
    const bytes = new Uint8Array(await new Response(stream).arrayBuffer())
    const names = listTarEntries(bytes)

    let go = false
    let rust = false
    let cpp = false
    for (const n of names) {
      if (n.includes('go.mod')) go = true
      else if (n.includes('Cargo.toml')) rust = true
      else if (n.includes('CMakeLists.txt') || n.endsWith('.cpp') || n.endsWith('.hpp')) cpp = true
    }
    if (go) return 'go'
    if (rust) return 'rust'
    if (cpp) return 'cpp'
    return 'unknown'
  } catch {
    // Not actually gzip/tar — let the caller report it.
    return null
  }
}

export const LANGUAGE_LABELS = { go: 'Go', rust: 'Rust', cpp: 'C++', unknown: 'Unknown' }

export function languageLabel(lang) {
  if (!lang) return '—'
  return LANGUAGE_LABELS[String(lang).toLowerCase()] ?? lang
}
