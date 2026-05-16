package config

import (
	"os"
	"strconv"
	"time"
)

const (
	defaultHTTPAddr        = ":8081"
	defaultRedisAddr       = "localhost:6379"
	defaultRedisKeyPattern = "test:*:latest"
	defaultStreamPeriodMS  = "1000"
)

type Config struct {
	HTTPAddr        string
	RedisAddr       string
	RedisKeyPattern string
	StreamPeriodMS  string
}

func Load() Config {
	return Config{
		HTTPAddr:        envOrDefault("LEADERBOARD_HTTP_ADDR", defaultHTTPAddr),
		RedisAddr:       envOrDefault("REDIS_ADDR", defaultRedisAddr),
		RedisKeyPattern: envOrDefault("REDIS_KEY_PATTERN", defaultRedisKeyPattern),
		StreamPeriodMS:  envOrDefault("LEADERBOARD_STREAM_PERIOD_MS", defaultStreamPeriodMS),
	}
}

func (c Config) StreamPeriod() time.Duration {
	ms, err := strconv.ParseUint(c.StreamPeriodMS, 10, 64)
	if err != nil || ms == 0 {
		ms, _ = strconv.ParseUint(defaultStreamPeriodMS, 10, 64)
	}
	return time.Duration(ms) * time.Millisecond
}

func envOrDefault(key, fallback string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return fallback
}
