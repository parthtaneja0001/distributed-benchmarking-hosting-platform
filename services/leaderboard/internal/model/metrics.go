package model

// LatestMetrics mirrors the JSON written by telemetry ingester v2.
type LatestMetrics struct {
	TestID   string  `json:"test_id"`
	WindowMS uint64  `json:"window_ms"`
	TPS      float64 `json:"tps"`
	Total    uint64  `json:"total"`
	Success  uint64  `json:"success"`
	Failure  uint64  `json:"failure"`
	P50US    *uint64 `json:"p50_us"`
	P90US    *uint64 `json:"p90_us"`
	P99US    *uint64 `json:"p99_us"`
}

type LeaderboardEntry struct {
	Rank    int     `json:"rank"`
	TestID  string  `json:"test_id"`
	Score   float64 `json:"score"`
	TPS     float64 `json:"tps"`
	Total   uint64  `json:"total"`
	Success uint64  `json:"success"`
	Failure uint64  `json:"failure"`
	P50US   *uint64 `json:"p50_us"`
	P90US   *uint64 `json:"p90_us"`
	P99US   *uint64 `json:"p99_us"`
}

type LeaderboardResponse struct {
	Entries []LeaderboardEntry `json:"entries"`
}
