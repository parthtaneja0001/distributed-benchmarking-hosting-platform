// Sample leaderboard data used only when no live backend is reachable,
// so the UI is meaningful in previews and demos. When VITE_WS_URL points
// at a real server, live data replaces this entirely.
const NAMES = [
  'lockfree-orderbook',
  'simd-matcher',
  'rust-hft-core',
  'zerocopy-engine',
  'cpp-latency-king',
  'go-market-maker',
  'async-arb-bot',
  'nano-ticker',
]

const LANGS = ['Rust', 'C++', 'Rust', 'C++', 'C++', 'Go', 'Rust', 'C']

function rand(min, max, decimals = 0) {
  const v = min + Math.random() * (max - min)
  return decimals ? Number(v.toFixed(decimals)) : Math.round(v)
}

function makeEntry(i) {
  const p50 = rand(4, 40)
  const p90 = p50 + rand(5, 30)
  const p99 = p90 + rand(10, 90)
  const tps = rand(120000, 950000)
  const correctness = rand(0.86, 1.0, 4)
  // Composite score rewards throughput + correctness, penalizes tail latency.
  const score = (tps / 1000) * correctness - p99 * 2
  return {
    test_id: `${NAMES[i % NAMES.length]}-${(i + 1).toString().padStart(2, '0')}${Math.random()
      .toString(16)
      .slice(2, 8)}`,
    language: LANGS[i % LANGS.length],
    tps,
    p50_us: p50,
    p90_us: p90,
    p99_us: p99,
    correctness,
    score,
  }
}

export function generateDemoEntries(n = 8) {
  const entries = Array.from({ length: n }, (_, i) => makeEntry(i))
  entries.sort((a, b) => b.score - a.score)
  return entries.map((e, idx) => ({ ...e, rank: idx + 1 }))
}

// Produce a lightly-mutated snapshot so the demo board "moves" like a live feed.
export function jitterEntries(entries) {
  const next = entries.map((e) => {
    const delta = () => 1 + (Math.random() - 0.5) * 0.06
    const tps = Math.round(e.tps * delta())
    const p50 = Math.max(2, Math.round(e.p50_us * delta()))
    const p90 = Math.max(p50 + 2, Math.round(e.p90_us * delta()))
    const p99 = Math.max(p90 + 2, Math.round(e.p99_us * delta()))
    const correctness = Math.min(1, Math.max(0.8, e.correctness + (Math.random() - 0.5) * 0.01))
    const score = (tps / 1000) * correctness - p99 * 2
    return { ...e, tps, p50_us: p50, p90_us: p90, p99_us: p99, correctness, score }
  })
  next.sort((a, b) => b.score - a.score)
  return next.map((e, idx) => ({ ...e, rank: idx + 1 }))
}

export function rerank(entries) {
  const next = [...entries].sort((a, b) => b.score - a.score)
  return next.map((e, idx) => ({ ...e, rank: idx + 1 }))
}

// Deterministic PRNG so a given submission always gets the same base numbers.
function seeded(seedStr) {
  let h = 1779033703 ^ seedStr.length
  for (let i = 0; i < seedStr.length; i++) {
    h = Math.imul(h ^ seedStr.charCodeAt(i), 3432918353)
    h = (h << 13) | (h >>> 19)
  }
  return () => {
    h = Math.imul(h ^ (h >>> 16), 2246822507)
    h = Math.imul(h ^ (h >>> 13), 3266489909)
    h ^= h >>> 16
    return (h >>> 0) / 4294967296
  }
}

const SUB_LANG_LABELS = { go: 'Go', rust: 'Rust', cpp: 'C++' }

/** Simulated benchmark result for a demo-mode upload. */
export function makeSubmissionEntry(sub) {
  const r = seeded(sub.id)
  const between = (min, max) => Math.round(min + r() * (max - min))
  const p50 = between(5, 35)
  const p90 = p50 + between(5, 25)
  const p99 = p90 + between(10, 70)
  const tps = between(150000, 900000)
  const correctness = Number((0.9 + r() * 0.1).toFixed(4))
  return {
    test_id: sub.name,
    language: SUB_LANG_LABELS[sub.language] ?? 'Unknown',
    tps,
    p50_us: p50,
    p90_us: p90,
    p99_us: p99,
    correctness,
    score: (tps / 1000) * correctness - p99 * 2,
    mine: true,
  }
}
