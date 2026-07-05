import { useEffect, useRef, useState } from 'react'
import { WS_URL, HAS_WS_BACKEND } from './config'
import { generateDemoEntries, jitterEntries } from './demoData'
import { IconGauge, IconZap, IconTarget, IconActivity } from './icons'

const COLUMNS = 8

function formatNum(n) {
  if (n == null) return '—'
  return n.toLocaleString('en-US')
}

function rankClass(rank) {
  if (rank === 1) return 'rank gold'
  if (rank === 2) return 'rank silver'
  if (rank === 3) return 'rank bronze'
  return 'rank'
}

function correctnessClass(c) {
  if (c >= 0.99) return 'bar-fill'
  if (c >= 0.95) return 'bar-fill warn'
  return 'bar-fill bad'
}

function Leaderboard() {
  const [entries, setEntries] = useState([])
  // status: 'connecting' | 'live' | 'offline' | 'demo'
  const [status, setStatus] = useState(HAS_WS_BACKEND ? 'connecting' : 'demo')
  const [updatedAt, setUpdatedAt] = useState(null)

  // Refs keep reconnect timers/sockets stable across StrictMode re-runs.
  const wsRef = useRef(null)
  const retryRef = useRef(null)
  const attemptsRef = useRef(0)
  const demoRef = useRef(null)

  useEffect(() => {
    let cancelled = false

    // Starts the simulated data stream. Idempotent — a running demo is reused.
    const startDemo = () => {
      if (demoRef.current) return
      let current = generateDemoEntries(8)
      setEntries(current)
      setUpdatedAt(new Date())
      setStatus('demo')
      demoRef.current = setInterval(() => {
        current = jitterEntries(current)
        setEntries(current)
        setUpdatedAt(new Date())
      }, 2000)
    }

    const stopDemo = () => {
      if (demoRef.current) {
        clearInterval(demoRef.current)
        demoRef.current = null
      }
    }

    // ---- No backend configured: demo only ----
    if (!HAS_WS_BACKEND) {
      startDemo()
      return () => stopDemo()
    }

    // ---- Backend configured: try live, fall back to demo if unreachable ----
    // If the first connection doesn't succeed quickly, show demo data instead
    // of an empty "Reconnecting…" board. We keep retrying in the background and
    // upgrade to live automatically once the backend responds.
    const fallbackTimer = setTimeout(() => {
      if (!cancelled) startDemo()
    }, 3000)

    const connect = () => {
      if (cancelled) return
      let ws
      try {
        ws = new WebSocket(WS_URL)
      } catch {
        scheduleRetry()
        return
      }
      wsRef.current = ws

      ws.onopen = () => {
        if (cancelled) return
        attemptsRef.current = 0
        stopDemo()
        setStatus('live')
      }

      ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data)
          const list = Array.isArray(data) ? data : data.entries
          if (Array.isArray(list)) {
            stopDemo()
            setStatus('live')
            setEntries(list)
            setUpdatedAt(new Date())
          }
        } catch (err) {
          console.error('[v0] Invalid JSON from leaderboard', err)
        }
      }

      ws.onerror = () => {
        // onclose will follow and trigger the retry.
        try {
          ws.close()
        } catch {
          /* noop */
        }
      }

      ws.onclose = () => {
        if (cancelled) return
        scheduleRetry()
      }
    }

    const scheduleRetry = () => {
      if (cancelled) return
      attemptsRef.current += 1
      // Once we've failed at least once, keep the demo data visible.
      startDemo()
      // Exponential backoff capped at 15s.
      const delay = Math.min(1000 * 2 ** (attemptsRef.current - 1), 15000)
      retryRef.current = setTimeout(connect, delay)
    }

    connect()

    return () => {
      cancelled = true
      clearTimeout(fallbackTimer)
      clearTimeout(retryRef.current)
      stopDemo()
      if (wsRef.current) {
        wsRef.current.onclose = null
        wsRef.current.onerror = null
        try {
          wsRef.current.close()
        } catch {
          /* noop */
        }
      }
    }
  }, [])

  const isDemo = status === 'demo'
  const hasData = entries.length > 0

  // Aggregate stats for the header cards.
  const leader = hasData ? entries.find((e) => e.rank === 1) || entries[0] : null
  const bestP99 = hasData ? Math.min(...entries.map((e) => e.p99_us ?? Infinity)) : null
  const peakTps = hasData ? Math.max(...entries.map((e) => e.tps ?? 0)) : null

  const statusPill = {
    live: { cls: 'live', label: 'Live', pulse: true },
    connecting: { cls: 'offline', label: 'Connecting…', pulse: false },
    offline: { cls: 'offline', label: 'Reconnecting…', pulse: false },
    demo: { cls: 'demo', label: 'Demo data', pulse: true },
  }[status]

  return (
    <>
      <div className="page-head">
        <h1>Live leaderboard</h1>
        <p>
          Every submitted engine ranked in real time by a composite score across
          throughput, tail latency, and correctness.
        </p>
      </div>

      <div className="stat-grid">
        <div className="stat">
          <div className="stat-label">
            <IconActivity width={13} height={13} /> Submissions
          </div>
          <div className="stat-value">{hasData ? entries.length : '—'}</div>
        </div>
        <div className="stat">
          <div className="stat-label">
            <IconTarget width={13} height={13} /> Top score
          </div>
          <div className="stat-value">{leader ? leader.score.toFixed(1) : '—'}</div>
        </div>
        <div className="stat">
          <div className="stat-label">
            <IconZap width={13} height={13} /> Peak throughput
          </div>
          <div className="stat-value">
            {peakTps ? (peakTps / 1000).toFixed(0) : '—'}
            <span className="unit">K TPS</span>
          </div>
        </div>
        <div className="stat">
          <div className="stat-label">
            <IconGauge width={13} height={13} /> Best p99
          </div>
          <div className="stat-value">
            {bestP99 != null && isFinite(bestP99) ? bestP99 : '—'}
            <span className="unit">µs</span>
          </div>
        </div>
      </div>

      <div className="lb-head">
        <div className="card-title">Rankings</div>
        <span className={`status-pill ${statusPill.cls}`}>
          <span className={`dot${statusPill.pulse ? ' pulse' : ''}`} />
          {statusPill.label}
        </span>
      </div>

      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th style={{ width: 60 }}>Rank</th>
              <th>Engine</th>
              <th>Lang</th>
              <th className="num">TPS</th>
              <th className="num">p50 µs</th>
              <th className="num">p90 µs</th>
              <th className="num">p99 µs</th>
              <th className="num">Correctness</th>
            </tr>
          </thead>
          <tbody>
            {!hasData && status !== 'demo' ? (
              status === 'connecting' ? (
                Array.from({ length: 5 }).map((_, i) => (
                  <tr key={i} className="skeleton-row">
                    {Array.from({ length: COLUMNS }).map((__, j) => (
                      <td key={j}>
                        <div className="skeleton" />
                      </td>
                    ))}
                  </tr>
                ))
              ) : (
                <tr className="empty-row">
                  <td colSpan={COLUMNS}>
                    No benchmark data yet. Submit an engine to populate the board.
                  </td>
                </tr>
              )
            ) : (
              entries.map((entry) => {
                const c = entry.correctness
                return (
                  <tr key={entry.test_id}>
                    <td>
                      <span className={rankClass(entry.rank)}>{entry.rank}</span>
                    </td>
                    <td className="mono" title={entry.test_id}>
                      {entry.test_id?.length > 22
                        ? entry.test_id.slice(0, 20) + '…'
                        : entry.test_id}
                    </td>
                    <td>{entry.language || '—'}</td>
                    <td className="num mono">{formatNum(entry.tps)}</td>
                    <td className="num mono">{entry.p50_us ?? '—'}</td>
                    <td className="num mono">{entry.p90_us ?? '—'}</td>
                    <td className="num mono">{entry.p99_us ?? '—'}</td>
                    <td className="num">
                      {c != null ? (
                        <span className="correctness">
                          <span className="bar">
                            <span
                              className={correctnessClass(c)}
                              style={{ width: `${Math.round(c * 100)}%` }}
                            />
                          </span>
                          <span className="mono">{(c * 100).toFixed(1)}%</span>
                        </span>
                      ) : (
                        '—'
                      )}
                    </td>
                  </tr>
                )
              })
            )}
          </tbody>
        </table>
      </div>

      <div className="lb-footer">
        <span>
          {isDemo
            ? HAS_WS_BACKEND
              ? 'Backend unreachable — showing simulated data. Retrying in the background…'
              : 'Showing simulated data — set VITE_WS_URL to stream live results.'
            : 'Score = throughput × correctness − tail-latency penalty.'}
        </span>
        {updatedAt && (
          <span>
            Updated{' '}
            {updatedAt.toLocaleTimeString('en-US', {
              hour12: false,
              hour: '2-digit',
              minute: '2-digit',
              second: '2-digit',
            })}
          </span>
        )}
      </div>
    </>
  )
}

export default Leaderboard
