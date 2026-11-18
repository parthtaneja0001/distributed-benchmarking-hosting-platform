package deployer

import (
	"context"
	"encoding/json"
	"log"
	"os"
	"time"

	goredis "github.com/redis/go-redis/v9"
)

// StatusWriter updates the submission:<id> record (written by the submission
// service) so the leaderboard can show building / running / failed.
type StatusWriter struct {
	client *goredis.Client
}

func NewStatusWriter() *StatusWriter {
	addr := os.Getenv("REDIS_ADDR")
	if addr == "" {
		addr = "localhost:6379"
	}
	return &StatusWriter{client: goredis.NewClient(&goredis.Options{Addr: addr})}
}

// Set merges status (and an optional error) into the record. Best effort.
func (s *StatusWriter) Set(id, status, errMsg string) {
	if s == nil {
		return
	}
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()

	key := "submission:" + id
	doc := map[string]any{}
	if raw, err := s.client.Get(ctx, key).Result(); err == nil {
		_ = json.Unmarshal([]byte(raw), &doc)
	}
	doc["id"] = id
	doc["status"] = status
	if errMsg != "" {
		if len(errMsg) > 300 {
			errMsg = errMsg[:300] + "…"
		}
		doc["error"] = errMsg
	} else {
		delete(doc, "error")
	}
	payload, _ := json.Marshal(doc)
	if err := s.client.Set(ctx, key, payload, 7*24*time.Hour).Err(); err != nil {
		log.Printf("status update %s=%s failed: %v", id, status, err)
	}
}
