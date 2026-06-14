import { useState } from 'react'
import Upload from './Upload'
import Leaderboard from './Leaderboard'
import './App.css'

function App() {
  const [page, setPage] = useState('upload')

  return (
    <div className="app">
      <nav className="nav">
        <button
          onClick={() => setPage('upload')}
          className={page === 'upload' ? 'active' : ''}
        >
          Upload
        </button>
        <button
          onClick={() => setPage('leaderboard')}
          className={page === 'leaderboard' ? 'active' : ''}
        >
          Leaderboard
        </button>
      </nav>

      <main className="main">
        {page === 'upload' ? <Upload /> : <Leaderboard />}
      </main>
    </div>
  )
}

export default App