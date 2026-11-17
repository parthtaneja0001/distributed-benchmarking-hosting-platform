import { useState } from 'react'
import Upload from './Upload'
import Leaderboard from './Leaderboard'
import { IconActivity, IconUpload, IconTrophy } from './icons'
import './App.css'

function App() {
  const [page, setPage] = useState('leaderboard')

  return (
    <div className="app">
      <header className="header">
        <div className="container header-inner">
          <div className="brand">
            <span className="brand-mark">
              <IconActivity width={17} height={17} />
            </span>
            QuantBench
            <span className="brand-sub">Trading Engine Benchmarks</span>
          </div>

          <nav className="nav" aria-label="Primary">
            <button
              onClick={() => setPage('leaderboard')}
              className={page === 'leaderboard' ? 'active' : ''}
              aria-current={page === 'leaderboard' ? 'page' : undefined}
            >
              <IconTrophy width={16} height={16} />
              Leaderboard
            </button>
            <button
              onClick={() => setPage('upload')}
              className={page === 'upload' ? 'active' : ''}
              aria-current={page === 'upload' ? 'page' : undefined}
            >
              <IconUpload width={16} height={16} />
              Submit Engine
            </button>
          </nav>
        </div>
      </header>

      <main className="main">
        <div className="container">
          {page === 'upload' ? (
            <Upload onViewLeaderboard={() => setPage('leaderboard')} />
          ) : (
            <Leaderboard />
          )}
        </div>
      </main>

      <footer className="footer">
        <div className="container footer-inner">
          <span>QuantBench — distributed benchmarking &amp; hosting platform</span>
          <span>Latency in microseconds · lower is better</span>
        </div>
      </footer>
    </div>
  )
}

export default App
