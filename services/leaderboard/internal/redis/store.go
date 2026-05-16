package redis

import (
	"context"
	"encoding/json"
	"fmt"

	goredis "github.com/redis/go-redis/v9"

	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/leaderboard/internal/model"
)

type Store struct {
	client     *goredis.Client
	keyPattern string
}

func NewStore(addr, keyPattern string) *Store {
	return &Store{
		client:     goredis.NewClient(&goredis.Options{Addr: addr}),
		keyPattern: keyPattern,
	}
}

func (s *Store) LatestMetrics(ctx context.Context) ([]model.LatestMetrics, error) {
	var cursor uint64
	metrics := make([]model.LatestMetrics, 0)

	for {
		keys, nextCursor, err := s.client.Scan(ctx, cursor, s.keyPattern, 100).Result()
		if err != nil {
			return nil, fmt.Errorf("scan Redis metrics keys: %w", err)
		}

		for _, key := range keys {
			item, err := s.readMetric(ctx, key)
			if err != nil {
				return nil, err
			}
			metrics = append(metrics, item)
		}

		cursor = nextCursor
		if cursor == 0 {
			break
		}
	}

	return metrics, nil
}

func (s *Store) readMetric(ctx context.Context, key string) (model.LatestMetrics, error) {
	payload, err := s.client.Get(ctx, key).Result()
	if err != nil {
		return model.LatestMetrics{}, fmt.Errorf("read Redis key %s: %w", key, err)
	}

	var metric model.LatestMetrics
	if err := json.Unmarshal([]byte(payload), &metric); err != nil {
		return model.LatestMetrics{}, fmt.Errorf("decode Redis key %s: %w", key, err)
	}

	return metric, nil
}
