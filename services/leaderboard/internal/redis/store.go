package redis

import (
	"context"
	"encoding/json"
	"errors"
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
			if errors.Is(err, goredis.Nil) {
				continue
			}
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

const submissionKeyPrefix = "submission:"

// Submissions returns all submission metadata records, keyed by id.
func (s *Store) Submissions(ctx context.Context) (map[string]model.Submission, error) {
	out := make(map[string]model.Submission)
	var cursor uint64
	for {
		keys, next, err := s.client.Scan(ctx, cursor, submissionKeyPrefix+"*", 100).Result()
		if err != nil {
			return nil, fmt.Errorf("scan Redis submission keys: %w", err)
		}
		if len(keys) > 0 {
			values, err := s.client.MGet(ctx, keys...).Result()
			if err != nil {
				return nil, fmt.Errorf("read Redis submission keys: %w", err)
			}
			for _, v := range values {
				str, ok := v.(string)
				if !ok {
					continue // key expired between SCAN and MGET
				}
				var sub model.Submission
				if json.Unmarshal([]byte(str), &sub) == nil && sub.ID != "" {
					out[sub.ID] = sub
				}
			}
		}
		cursor = next
		if cursor == 0 {
			break
		}
	}
	return out, nil
}
