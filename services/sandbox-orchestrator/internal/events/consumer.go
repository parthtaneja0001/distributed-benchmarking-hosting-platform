package events

import (
    "context"
    "encoding/json"
    "fmt"
    "log"

    "github.com/segmentio/kafka-go"
)

// ProcessFunc is called for every received submission.
type ProcessFunc func(event SubmissionCreated) error

func ConsumeSubmissions(ctx context.Context, broker string, process ProcessFunc) {
    reader := kafka.NewReader(kafka.ReaderConfig{
        Brokers: []string{broker},
        Topic:   "submission.created",
        GroupID: "sandbox-orchestrator",
    })
    defer reader.Close()

    for {
        msg, err := reader.FetchMessage(ctx)
        if err != nil {
            log.Printf("Error fetching message: %v", err)
            continue
        }

        var event SubmissionCreated
        if err := json.Unmarshal(msg.Value, &event); err != nil {
            log.Printf("Error decoding event: %v", err)
            continue
        }

        fmt.Printf("Received submission: %s (lang: %s, file: %s)\n", event.ID, event.Language, event.ObjectKey)

        // Call the process function (will be deployer.DeployMock from main)
        if err := process(event); err != nil {
            log.Printf("Processing failed: %v", err)
            continue
        }

        if err := reader.CommitMessages(ctx, msg); err != nil {
            log.Printf("Error committing message: %v", err)
        }
    }
}