package scoring

import "github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/model"

type Scorer interface {
	Score(metric model.LatestMetrics) float64
}

// ProvisionalScorer: throughput × correctness, minus a p99 latency penalty
// and a per-failure penalty. Missing correctness is treated as 1.0.
type ProvisionalScorer struct{}

func (ProvisionalScorer) Score(metric model.LatestMetrics) float64 {
	correctness := 1.0
	if metric.Correctness != nil {
		correctness = *metric.Correctness
	}

	p99Penalty := 0.0
	if metric.P99US != nil {
		p99Penalty = float64(*metric.P99US) / 1_000.0
	}

	failurePenalty := float64(metric.Failure) * 10.0
	return metric.TPS*correctness - p99Penalty - failurePenalty
}
