import { useState } from 'react'

const UPLOAD_URL = import.meta.env.VITE_UPLOAD_URL

function Upload() {
  const [file, setFile] = useState(null)
  const [result, setResult] = useState(null)
  const [error, setError] = useState(null)
  const [loading, setLoading] = useState(false)

  const handleSubmit = (e) => {
    e.preventDefault()
    if (!file) return

    setLoading(true)
    setError(null)
    setResult(null)

    const xhr = new XMLHttpRequest()
    xhr.open('POST', UPLOAD_URL)

    xhr.onload = () => {
      try {
        const data = JSON.parse(xhr.responseText)
        if (xhr.status === 200) {
          setResult(data)
        } else {
          setError(data.error || 'Upload failed')
        }
      } catch (err) {
        setError(err.message)
      }
      setLoading(false)
    }

    xhr.onerror = () => {
      setError('Network error')
      setLoading(false)
    }

    const formData = new FormData()
    formData.append('file', file)
    xhr.send(formData)
  }

  return (
    <div className="glass-card">
      <h2>Submit Your Trading Engine</h2>
      <form onSubmit={handleSubmit} className="upload-form">
        <div className="file-input-wrapper">
          <label className="file-input-label">
            {file ? 'Change file' : 'Select tarball (.tar.gz)'}
          </label>
          <input
            type="file"
            accept=".tar.gz,application/gzip"
            onChange={(e) => setFile(e.target.files[0])}
          />
        </div>
        {file && <span className="file-name">{file.name}</span>}

        <button type="submit" className="btn" disabled={loading}>
          {loading ? 'Uploading...' : 'Run Benchmark'}
        </button>
      </form>

      {result && (
        <div className="result-badge" style={{ marginTop: '1.5rem' }}>
          <span>Submission ID: <strong>{result.id?.slice(0, 8)}...</strong></span>
          <span>Language: <strong>{result.language || 'detected'}</strong></span>
        </div>
      )}

      {error && (
        <div className="error-badge" style={{ marginTop: '1.5rem' }}>
          {error}
        </div>
      )}
    </div>
  )
}

export default Upload