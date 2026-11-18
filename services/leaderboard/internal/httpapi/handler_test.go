package httpapi

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/leaderboard"
	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/model"
	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/scoring"
)

type fakeMetricsStore struct {
	items []model.LatestMetrics
}

func (s fakeMetricsStore) LatestMetrics(context.Context) ([]model.LatestMetrics, error) {
	return s.items, nil
}

func TestLeaderboardRanksByScoreDescending(t *testing.T) {
	p99Fast := uint64(1_000)
	p99Slow := uint64(5_000)
	service := leaderboard.NewService(fakeMetricsStore{
		items: []model.LatestMetrics{
			{TestID: "slow", TPS: 100, P99US: &p99Slow},
			{TestID: "fast", TPS: 100, P99US: &p99Fast},
		},
	}, scoring.ProvisionalScorer{})
	handler := NewHandler(service, http.NotFoundHandler())

	req := httptest.NewRequest(http.MethodGet, "/leaderboard", nil)
	rec := httptest.NewRecorder()

	handler.Routes().ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d, want %d", rec.Code, http.StatusOK)
	}

	var response model.LeaderboardResponse
	if err := json.NewDecoder(rec.Body).Decode(&response); err != nil {
		t.Fatalf("decode response: %v", err)
	}

	if len(response.Entries) != 2 {
		t.Fatalf("entries = %d, want 2", len(response.Entries))
	}
	if response.Entries[0].TestID != "fast" || response.Entries[0].Rank != 1 {
		t.Fatalf("top entry = %+v, want fast rank 1", response.Entries[0])
	}
}

type fakeStoreWithSubs struct {
	fakeMetricsStore
	subs map[string]model.Submission
}

func (s fakeStoreWithSubs) Submissions(context.Context) (map[string]model.Submission, error) {
	return s.subs, nil
}

func TestLeaderboardMergesSubmissionMetadataAndPending(t *testing.T) {
	c := 0.98
	service := leaderboard.NewService(fakeStoreWithSubs{
		fakeMetricsStore: fakeMetricsStore{items: []model.LatestMetrics{
			{TestID: "done", TPS: 500, Correctness: &c},
		}},
		subs: map[string]model.Submission{
			"done":  {ID: "done", Name: "engine-a", Language: "go"},
			"build": {ID: "build", Name: "engine-b", Language: "rust", Status: "building"},
		},
	}, scoring.ProvisionalScorer{})

	resp, err := service.Current(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(resp.Entries) != 1 || resp.Entries[0].Name != "engine-a" || resp.Entries[0].Language != "go" {
		t.Fatalf("entries = %+v", resp.Entries)
	}
	if resp.Entries[0].Correctness == nil || *resp.Entries[0].Correctness != c {
		t.Fatalf("correctness not passed through: %+v", resp.Entries[0])
	}
	if len(resp.Pending) != 1 || resp.Pending[0].ID != "build" {
		t.Fatalf("pending = %+v", resp.Pending)
	}
}
