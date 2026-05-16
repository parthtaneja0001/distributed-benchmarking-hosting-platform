package deployer

import (
    "context"
    "fmt"
    "time"

    "github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/sandbox-orchestrator/internal/events"
    "github.com/segmentio/kafka-go"
)

// DeployMock simulates deploying a submission.
func DeployMock(event events.SubmissionCreated) error {
    fmt.Printf("Mock deploying submission %s (language: %s)...\n", event.ID, event.Language)
    time.Sleep(2 * time.Second)
	return publishSandboxReady(event.ID, "ws://localhost:8080/ws")
}

func publishSandboxReady(submissionID, endpoint string) error {
    writer := &kafka.Writer{
        Addr:     kafka.TCP("localhost:9092"),
        Topic:    "sandbox.ready",
        Balancer: &kafka.LeastBytes{},
    }
    defer writer.Close()

    payload := fmt.Sprintf(`{"submission_id":"%s","endpoint":"%s"}`, submissionID, endpoint)
    msg := kafka.Message{
        Key:   []byte(submissionID),
        Value: []byte(payload),
    }

    ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
    defer cancel()

    err := writer.WriteMessages(ctx, msg)
    if err != nil {
        return fmt.Errorf("failed to publish sandbox.ready: %w", err)
    }
    fmt.Printf("Published sandbox.ready for %s at %s\n", submissionID, endpoint)
    return nil
}