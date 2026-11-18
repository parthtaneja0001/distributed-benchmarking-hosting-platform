package model

// LatestMetrics mirrors the JSON written by telemetry ingester v2.
type LatestMetrics struct {
	TestID      string   `json:"test_id"`
	WindowMS    uint64   `json:"window_ms"`
	TPS         float64  `json:"tps"`
	Total       uint64   `json:"total"`
	Success     uint64   `json:"success"`
	Failure     uint64   `json:"failure"`
	P50US       *uint64  `json:"p50_us"`
	P90US       *uint64  `json:"p90_us"`
	P99US       *uint64  `json:"p99_us"`
	Correctness *float64 `json:"correctness"`
}

// Submission is the metadata written to Redis (key submission:<id>) by the
// submission service and updated by the sandbox orchestrator.
type Submission struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Language    string `json:"language"`
	Status      string `json:"status"` // queued | building | running | failed
	Error       string `json:"error,omitempty"`
	SubmittedAt int64  `json:"submitted_at"` // unix millis
}

type LeaderboardEntry struct {
	Rank        int      `json:"rank"`
	TestID      string   `json:"test_id"`
	Name        string   `json:"name,omitempty"`
	Language    string   `json:"language,omitempty"`
	Score       float64  `json:"score"`
	TPS         float64  `json:"tps"`
	Total       uint64   `json:"total"`
	Success     uint64   `json:"success"`
	Failure     uint64   `json:"failure"`
	P50US       *uint64  `json:"p50_us"`
	P90US       *uint64  `json:"p90_us"`
	P99US       *uint64  `json:"p99_us"`
	Correctness *float64 `json:"correctness"`
}

type LeaderboardResponse struct {
	Entries []LeaderboardEntry `json:"entries"`
	// Submissions that have no benchmark results yet (queued, building,
	// running, or failed).
	Pending []Submission `json:"pending"`
}
