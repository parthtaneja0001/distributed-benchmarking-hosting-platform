package config

import "os"

const (
	defaultKafkaBroker         = "localhost:9092"
	defaultSandboxMockEndpoint = "ws://localhost:8080/ws"
)

// Config contains runtime settings for the sandbox orchestrator.
type Config struct {
	KafkaBroker         string
	SandboxMockEndpoint string
}

// Load reads configuration from environment variables and applies local-safe
// defaults for the current mock pipeline.
func Load() Config {
	return Config{
		KafkaBroker:         envOrDefault("KAFKA_BROKER", defaultKafkaBroker),
		SandboxMockEndpoint: envOrDefault("SANDBOX_MOCK_ENDPOINT", defaultSandboxMockEndpoint),
	}
}

func envOrDefault(key, fallback string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return fallback
}
