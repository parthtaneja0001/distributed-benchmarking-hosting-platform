package deployer

import (
	"fmt"
	"time"

	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/sandbox-orchestrator/internal/events"
)

// Deployer is the interface that both mock and real deployers implement.
type Deployer interface {
	Deploy(event events.SubmissionCreated) error
}

// MockDeployer simulates deployment (for quick tests without Docker).
type MockDeployer struct {
	pub      *SandboxReadyPublisher
	endpoint string
}

func NewMockDeployer(pub *SandboxReadyPublisher, endpoint string) *MockDeployer {
	return &MockDeployer{pub: pub, endpoint: endpoint}
}

func (m *MockDeployer) Deploy(event events.SubmissionCreated) error {
	fmt.Printf("Mock deploying submission %s (language: %s)...\n", event.ID, event.Language)
	time.Sleep(2 * time.Second)
	return m.pub.Publish(event.ID, m.endpoint)
}

// --- SandboxReadyPublisher and publishSandboxReady remain unchanged ---
// (keep the existing SandboxReadyPublisher struct, NewSandboxReadyPublisher, and Publish method)