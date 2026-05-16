package main

import (
	"context"
	"log"
	"os"
	"os/signal"

	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/sandbox-orchestrator/internal/config"
	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/sandbox-orchestrator/internal/deployer"
	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/sandbox-orchestrator/internal/events"
)

func main() {
	cfg := config.Load()

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Graceful shutdown on Ctrl+C
	sigs := make(chan os.Signal, 1)
	signal.Notify(sigs, os.Interrupt)
	go func() {
		<-sigs
		cancel()
	}()

	publisher := deployer.NewSandboxReadyPublisher(cfg.KafkaBroker)
	mockDeployer := deployer.NewMockDeployer(cfg.SandboxMockEndpoint, publisher)

	log.Printf("Sandbox orchestrator consuming from %s", cfg.KafkaBroker)
	log.Printf("Mock sandbox endpoint: %s", cfg.SandboxMockEndpoint)

	events.ConsumeSubmissions(ctx, cfg.KafkaBroker, mockDeployer.Deploy)

	log.Println("Orchestrator shutting down")
}
