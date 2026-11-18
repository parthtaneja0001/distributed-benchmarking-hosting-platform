import { useState } from 'react'
import { UPLOAD_URL, HAS_UPLOAD_BACKEND } from './config'
import { detectLanguage, languageLabel } from './detectLanguage'
import { addSubmission, rememberMySubmission } from './demoStore'
import {
  IconUpload,
  IconFile,
  IconX,
  IconCheck,
  IconAlert,
} from './icons'

const MAX_BYTES = 50 * 1024 * 1024

function formatBytes(bytes) {
  if (!bytes) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(i === 0 ? 0 : 1)} ${units[i]}`
}

function randomId() {
  return (
    Math.random().toString(16).slice(2) + Math.random().toString(16).slice(2)
  ).slice(0, 32)
}

function Upload({ onViewLeaderboard }) {
  const [file, setFile] = useState(null)
  const [result, setResult] = useState(null)
  const [error, setError] = useState(null)
  const [loading, setLoading] = useState(false)
  const [dragging, setDragging] = useState(false)

  const selectFile = (f) => {
    if (!f) return
    setResult(null)
    // Drag-and-drop bypasses the input's `accept` filter, so validate here.
    if (!/\.(tar\.gz|tgz)$/i.test(f.name)) {
      setFile(null)
      setError(`"${f.name}" is not a .tar.gz archive.`)
      return
    }
    if (f.size > MAX_BYTES) {
      setFile(null)
      setError(`File is ${formatBytes(f.size)} — the limit is 50 MB.`)
      return
    }
    setFile(f)
    setError(null)
  }

  const clearFile = () => {
    setFile(null)
    setResult(null)
    setError(null)
  }

  // Demo submission used when no backend is configured: detect the language
  // in the browser (same rules as the backend) and queue a simulated run that
  // shows up on the leaderboard.
  const simulateSubmit = async () => {
    setLoading(true)
    setError(null)
    setResult(null)
    const [language] = await Promise.all([
      detectLanguage(file),
      new Promise((r) => setTimeout(r, 600)),
    ])
    if (language === null) {
      setError(`"${file.name}" could not be read as a gzipped tar archive.`)
      setLoading(false)
      return
    }
    const id = randomId()
    const base = file.name.replace(/\.(tar\.gz|tgz)$/i, '')
    addSubmission({ id, name: `${base}-${id.slice(0, 6)}`, language, submittedAt: Date.now() })
    setResult({ id, language, demo: true })
    setLoading(false)
  }

  const handleSubmit = (e) => {
    e.preventDefault()
    if (!file) return

    // Graceful degradation: without a real endpoint, avoid POSTing to "undefined".
    if (!HAS_UPLOAD_BACKEND) {
      simulateSubmit()
      return
    }

    setLoading(true)
    setError(null)
    setResult(null)

    const xhr = new XMLHttpRequest()
    xhr.open('POST', UPLOAD_URL)

    xhr.onload = () => {
      let data = null
      try {
        data = JSON.parse(xhr.responseText)
      } catch {
        /* non-JSON body (e.g. a proxy error page) */
      }
      if (xhr.status >= 200 && xhr.status < 300) {
        if (data && data.id) {
          rememberMySubmission(data.id)
          setResult(data)
        }
        else setError('Received an invalid response from the server.')
      } else {
        setError(data?.error || `Upload failed (HTTP ${xhr.status})`)
      }
      setLoading(false)
    }

    xhr.onerror = () => {
      setError('Network error — could not reach the upload service.')
      setLoading(false)
    }

    const formData = new FormData()
    formData.append('file', file)
    xhr.send(formData)
  }

  return (
    <>
      <div className="page-head">
        <h1>Submit your trading engine</h1>
        <p>
          Upload a packaged engine and we&apos;ll build it in an isolated sandbox, replay
          a deterministic market feed, and rank it on latency, throughput, and
          correctness against every other submission.
        </p>
      </div>

      <div className="card">
        <div className="card-title">Engine tarball</div>
        <div className="card-desc">
          Accepted format: <code style={{ fontFamily: 'var(--mono)' }}>.tar.gz</code> containing
          your source and a build manifest.
        </div>

        <form onSubmit={handleSubmit}>
          <label
            className={`dropzone${dragging ? ' dragging' : ''}`}
            onDragOver={(e) => {
              e.preventDefault()
              setDragging(true)
            }}
            onDragLeave={() => setDragging(false)}
            onDrop={(e) => {
              e.preventDefault()
              setDragging(false)
              selectFile(e.dataTransfer.files?.[0])
            }}
            style={{ marginTop: '1.25rem' }}
          >
            <span className="dropzone-icon">
              <IconUpload width={20} height={20} />
            </span>
            <span className="dropzone-title">
              {dragging ? 'Drop to attach' : 'Drag & drop or click to browse'}
            </span>
            <span className="dropzone-hint">Maximum 50 MB · .tar.gz</span>
            <input
              type="file"
              accept=".tar.gz,.tgz,application/gzip,application/x-gzip"
              onChange={(e) => {
                selectFile(e.target.files?.[0])
                // Reset so re-selecting the same file still fires onChange.
                e.target.value = ''
              }}
            />
          </label>

          {file && (
            <div className="file-chip">
              <span className="file-chip-icon">
                <IconFile width={16} height={16} />
              </span>
              <span className="file-chip-meta">
                <span className="file-chip-name">{file.name}</span>
                <span className="file-chip-size">{formatBytes(file.size)}</span>
              </span>
              <button type="button" onClick={clearFile} aria-label="Remove file">
                <IconX width={16} height={16} />
              </button>
            </div>
          )}

          <button type="submit" className="btn btn-block" disabled={loading || !file}>
            {loading ? (
              <>
                <span className="spinner" /> Submitting…
              </>
            ) : (
              <>
                <IconUpload width={16} height={16} /> Run benchmark
              </>
            )}
          </button>
        </form>

        {result && (
          <div className="alert alert-success">
            <span className="alert-icon">
              <IconCheck width={18} height={18} />
            </span>
            <div>
              <strong>Submission queued{result.demo ? ' (demo)' : ''}</strong>
              <div className="result-meta">
                <span className="result-meta-item">
                  ID<code>{(result.id || '').slice(0, 12)}…</code>
                </span>
                <span className="result-meta-item">
                  Language<code>{languageLabel(result.language)}</code>
                </span>
                <span className="result-meta-item">
                  Status<code>{result.demo ? 'benchmarking' : 'queued'}</code>
                </span>
              </div>
              {onViewLeaderboard && (
                <div className="result-actions">
                  <button type="button" className="btn btn-ghost" onClick={onViewLeaderboard}>
                    View on leaderboard
                  </button>
                </div>
              )}
            </div>
          </div>
        )}

        {error && (
          <div className="alert alert-error">
            <span className="alert-icon">
              <IconAlert width={18} height={18} />
            </span>
            <div>
              <strong>Submission failed</strong>
              <div style={{ marginTop: '0.2rem', color: 'var(--danger)' }}>{error}</div>
            </div>
          </div>
        )}
      </div>

      <div className="card">
        <div className="card-title">What happens next</div>
        <ol className="steps">
          <li className="step">
            <span className="step-num">1</span>
            <span>
              <strong>Build.</strong> Your tarball is unpacked and compiled in a
              network-isolated container.
            </span>
          </li>
          <li className="step">
            <span className="step-num">2</span>
            <span>
              <strong>Replay.</strong> A deterministic order-flow trace is streamed to
              your engine across distributed workers.
            </span>
          </li>
          <li className="step">
            <span className="step-num">3</span>
            <span>
              <strong>Measure.</strong> We record p50 / p90 / p99 latency, sustained
              throughput, and correctness against the reference matcher.
            </span>
          </li>
          <li className="step">
            <span className="step-num">4</span>
            <span>
              <strong>Rank.</strong> Results stream to the live leaderboard the moment
              the run completes.
            </span>
          </li>
        </ol>
      </div>
    </>
  )
}

export default Upload
