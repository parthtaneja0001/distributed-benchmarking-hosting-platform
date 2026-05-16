package deployer

import (
	"fmt"
	"time"

	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/sandbox-orchestrator/internal/events"
)

// MockDeployer simulates deployment and announces a configured test endpoint.
type MockDeployer struct {
	endpoint  string
	publisher *SandboxReadyPublisher
}

func NewMockDeployer(endpoint string, publisher *SandboxReadyPublisher) *MockDeployer {
	return &MockDeployer{
		endpoint:  endpoint,
		publisher: publisher,
	}
}

func (d *MockDeployer) Deploy(event events.SubmissionCreated) error {
	fmt.Printf("Mock deploying submission %s (language: %s)...\n", event.ID, event.Language)
	time.Sleep(2 * time.Second)
	return d.publisher.Publish(event.ID, d.endpoint)
}
