package leaderboard

import (
	"context"
	"sort"

	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/model"
	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/scoring"
)

type MetricsStore interface {
	LatestMetrics(ctx context.Context) ([]model.LatestMetrics, error)
}

type Service struct {
	store  MetricsStore
	scorer scoring.Scorer
}

func NewService(store MetricsStore, scorer scoring.Scorer) *Service {
	return &Service{
		store:  store,
		scorer: scorer,
	}
}

func (s *Service) Current(ctx context.Context) (model.LeaderboardResponse, error) {
	metrics, err := s.store.LatestMetrics(ctx)
	if err != nil {
		return model.LeaderboardResponse{}, err
	}

	entries := make([]model.LeaderboardEntry, 0, len(metrics))
	for _, metric := range metrics {
		entries = append(entries, model.LeaderboardEntry{
			TestID:  metric.TestID,
			Score:   s.scorer.Score(metric),
			TPS:     metric.TPS,
			Total:   metric.Total,
			Success: metric.Success,
			Failure: metric.Failure,
			P50US:   metric.P50US,
			P90US:   metric.P90US,
			P99US:   metric.P99US,
		})
	}

	sort.SliceStable(entries, func(i, j int) bool {
		return entries[i].Score > entries[j].Score
	})

	for i := range entries {
		entries[i].Rank = i + 1
	}

	return model.LeaderboardResponse{Entries: entries}, nil
}
