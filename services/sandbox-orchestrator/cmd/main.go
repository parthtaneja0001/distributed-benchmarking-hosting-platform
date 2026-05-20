package main

import (
	"context"
	"log"
	"os"
	"os/signal"
	"strings"

	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/sandbox-orchestrator/internal/config"
	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/sandbox-orchestrator/internal/deployer"
	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/sandbox-orchestrator/internal/events"
)

func main() {
	cfg := config.Load()
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	sigs := make(chan os.Signal, 1)
	signal.Notify(sigs, os.Interrupt)
	go func() {
		<-sigs
		cancel()
	}()

	pub := deployer.NewSandboxReadyPublisher(cfg.KafkaBroker)

	// Select deployer: set DEPLOY_MODE=docker to use real containers
	var d deployer.Deployer
	mode := strings.ToLower(os.Getenv("DEPLOY_MODE"))
	if mode == "docker" {
		dd, err := deployer.NewDockerDeployer(pub)
		if err != nil {
			log.Fatalf("Failed to create Docker deployer: %v", err)
		}
		d = dd
		log.Println("Using Docker deployer")
	} else {
		d = deployer.NewMockDeployer(pub, cfg.SandboxMockEndpoint)
		log.Println("Using Mock deployer")
	}

	events.ConsumeSubmissions(ctx, cfg.KafkaBroker, d.Deploy)
	log.Println("Orchestrator shutting down")
}