import { useState, useEffect } from 'react'

const WS_URL = import.meta.env.VITE_WS_URL

function Leaderboard() {
  const [entries, setEntries] = useState([])
  const [connected, setConnected] = useState(false)

  useEffect(() => {
    const ws = new WebSocket(WS_URL)

    ws.onopen = () => setConnected(true)

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data)
        if (data.entries) {
          setEntries(data.entries)
        }
      } catch (err) {
        console.error('Invalid JSON from leaderboard', err)
      }
    }

    ws.onerror = () => setConnected(false)
    ws.onclose = () => setConnected(false)

    return () => ws.close()
  }, [])

  return (
    <div className="glass-card">
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '1rem' }}>
        <h2 style={{ margin: 0 }}>Live Leaderboard</h2>
        <span style={{ fontSize: '0.8rem', color: connected ? '#60c080' : '#c06060' }}>
          {connected ? '● Live' : '○ Disconnected'}
        </span>
      </div>

      <table>
        <thead>
          <tr>
            <th>Rank</th>
            <th>Test ID</th>
            <th>Score</th>
            <th>TPS</th>
            <th>P50 µs</th>
            <th>P90 µs</th>
            <th>P99 µs</th>
            <th>Correctness</th>
          </tr>
        </thead>
        <tbody>
          {entries.length === 0 ? (
            <tr className="empty-row">
              <td colSpan="8">Waiting for benchmark data...</td>
            </tr>
          ) : (
            entries.map((entry) => (
              <tr key={entry.test_id}>
                <td><strong>#{entry.rank}</strong></td>
                <td>{entry.test_id?.slice(0, 8)}...</td>
                <td>{entry.score?.toFixed(2)}</td>
                <td>{entry.tps}</td>
                <td>{entry.p50_us ?? '-'}</td>
                <td>{entry.p90_us ?? '-'}</td>
                <td>{entry.p99_us ?? '-'}</td>
                <td>
                  {entry.correctness != null
                    ? (entry.correctness * 100).toFixed(1) + '%'
                    : '-'}
                </td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  )
}

export default Leaderboard