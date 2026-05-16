package deployer

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	"github.com/segmentio/kafka-go"
)

const sandboxReadyTopic = "sandbox.ready"

// SandboxReadyPublisher publishes sandbox lifecycle events to Kafka.
type SandboxReadyPublisher struct {
	broker string
}

type sandboxReadyEvent struct {
	SubmissionID string `json:"submission_id"`
	Endpoint     string `json:"endpoint"`
}

func NewSandboxReadyPublisher(broker string) *SandboxReadyPublisher {
	return &SandboxReadyPublisher{broker: broker}
}

func (p *SandboxReadyPublisher) Publish(submissionID, endpoint string) error {
	writer := &kafka.Writer{
		Addr:     kafka.TCP(p.broker),
		Topic:    sandboxReadyTopic,
		Balancer: &kafka.LeastBytes{},
	}
	defer writer.Close()

	payload, err := json.Marshal(sandboxReadyEvent{
		SubmissionID: submissionID,
		Endpoint:     endpoint,
	})
	if err != nil {
		return fmt.Errorf("failed to encode sandbox.ready event: %w", err)
	}

	msg := kafka.Message{
		Key:   []byte(submissionID),
		Value: payload,
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	if err := writer.WriteMessages(ctx, msg); err != nil {
		return fmt.Errorf("failed to publish sandbox.ready: %w", err)
	}

	fmt.Printf("Published sandbox.ready for %s at %s\n", submissionID, endpoint)
	return nil
}
