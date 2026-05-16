package scoring

import "github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/model"

type Scorer interface {
	Score(metric model.LatestMetrics) float64
}

// ProvisionalScorer ranks by throughput while penalizing p99 latency and failures.
// Correctness will become a major score input after validator work lands.
type ProvisionalScorer struct{}

func (ProvisionalScorer) Score(metric model.LatestMetrics) float64 {
	p99Penalty := 0.0
	if metric.P99US != nil {
		p99Penalty = float64(*metric.P99US) / 1_000.0
	}

	failurePenalty := float64(metric.Failure) * 10.0
	return metric.TPS - p99Penalty - failurePenalty
}
