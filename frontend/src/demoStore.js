// Submissions made while no upload backend is configured (demo mode).
// Kept in memory, mirrored to localStorage so they survive a reload, and
// broadcast via a window event so the leaderboard picks them up.

const KEY = 'quantbench.demoSubmissions'
const EVENT = 'quantbench:submissions'
const MAX = 20

// How long a demo submission shows as "Benchmarking…" before it gets results.
export const BENCHMARK_MS = 6000

function readStorage() {
  try {
    const v = JSON.parse(localStorage.getItem(KEY))
    return Array.isArray(v) ? v : []
  } catch {
    return []
  }
}

let submissions = readStorage()

export function loadSubmissions() {
  return submissions
}

export function addSubmission(sub) {
  submissions = [...submissions, sub].slice(-MAX)
  try {
    localStorage.setItem(KEY, JSON.stringify(submissions))
  } catch {
    /* storage unavailable — in-memory copy still works this session */
  }
  window.dispatchEvent(new CustomEvent(EVENT))
}

export function onSubmissionsChange(cb) {
  const onStorage = (e) => {
    if (e.key === KEY) {
      submissions = readStorage()
      cb()
    }
  }
  window.addEventListener(EVENT, cb)
  window.addEventListener('storage', onStorage)
  return () => {
    window.removeEventListener(EVENT, cb)
    window.removeEventListener('storage', onStorage)
  }
}
