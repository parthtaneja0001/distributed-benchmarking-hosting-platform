package scoring

import (
	"testing"

	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/model"
)

func TestProvisionalScorerPenalizesLatencyAndFailures(t *testing.T) {
	p99 := uint64(2_500)
	score := ProvisionalScorer{}.Score(model.LatestMetrics{
		TPS:     1000,
		Failure: 2,
		P99US:   &p99,
	})

	want := 977.5
	if score != want {
		t.Fatalf("score = %v, want %v", score, want)
	}
}
