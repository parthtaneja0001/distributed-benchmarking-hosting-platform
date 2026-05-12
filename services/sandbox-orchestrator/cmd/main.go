package main

import (
    "context"
    "log"
    "os"
    "os/signal"

    "github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/sandbox-orchestrator/internal/deployer"
    "github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/sandbox-orchestrator/internal/events"
)

func main() {
    ctx, cancel := context.WithCancel(context.Background())
    defer cancel()

    // Graceful shutdown on Ctrl+C
    sigs := make(chan os.Signal, 1)
    signal.Notify(sigs, os.Interrupt)
    go func() {
        <-sigs
        cancel()
    }()

    broker := "localhost:9092"
    events.ConsumeSubmissions(ctx, broker, deployer.DeployMock)

    log.Println("Orchestrator shutting down")
}