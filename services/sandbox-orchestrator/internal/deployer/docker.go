package deployer

import (
	"archive/tar"
	"bytes"
	"context"
	"fmt"
	"io"
	"log"
	"os"
	"path/filepath"
	"time"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/container"
	"github.com/docker/docker/client"
	"github.com/docker/go-connections/nat"
	"github.com/minio/minio-go/v7"
	"github.com/minio/minio-go/v7/pkg/credentials"

	"github.com/parthtaneja0001/distributed-benchmarking-hosting-platform/services/sandbox-orchestrator/internal/events"
)

// ---------- language profiles ----------

type languageProfile struct {
	BaseImage string
	BuildCmd  string // command that produces /engine binary
}

var profiles = map[string]languageProfile{
	"go": {
		BaseImage: "golang:1.22-alpine",
		BuildCmd:  "go build -o /engine .",
	},
	"rust": {
		BaseImage: "rust:1.80-alpine",
		BuildCmd:  "cargo build --release && find target/release/ -maxdepth 1 -type f -executable -exec cp {} /engine \\;",
	},
	"cpp": {
		BaseImage: "gcc:latest",
		BuildCmd:  "g++ -O2 -o /engine *.cpp",
	},
	"unknown": {
		BaseImage: "alpine:latest",
		BuildCmd:  "echo 'Error: unknown language' && exit 1",
	},
}

// ---------- Docker deployer ----------

type DockerDeployer struct {
	client *client.Client
	pub    *SandboxReadyPublisher
}

func NewDockerDeployer(pub *SandboxReadyPublisher) (*DockerDeployer, error) {
	cli, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation())
	if err != nil {
		return nil, fmt.Errorf("failed to create Docker client: %w", err)
	}
	return &DockerDeployer{client: cli, pub: pub}, nil
}

// Deploy builds and runs the submission, then publishes sandbox.ready.
func (d *DockerDeployer) Deploy(event events.SubmissionCreated) error {
	ctx := context.Background()
	subID := event.ID
	objectKey := event.ObjectKey // MinIO object key, e.g. <uuid>/submission.tar.gz
	lang := event.Language

	profile, ok := profiles[lang]
	if !ok {
		profile = profiles["unknown"]
	}
	log.Printf("Deploying submission %s (lang: %s) object key: %s", subID, lang, objectKey)

	// 1. Download the tarball from MinIO
	localTarball, err := downloadFromMinio(objectKey)
	if err != nil {
		return fmt.Errorf("download from MinIO failed: %w", err)
	}
	defer os.Remove(localTarball)

	// 2. Build the container image
	tag := fmt.Sprintf("submission-%s:latest", subID)
	if err := d.buildImage(ctx, localTarball, tag, profile); err != nil {
		return fmt.Errorf("build failed: %w", err)
	}

	// 3. Run the container
	hostPort, err := d.runContainer(ctx, tag, subID)
	if err != nil {
		return fmt.Errorf("run failed: %w", err)
	}

	endpoint := fmt.Sprintf("ws://localhost:%d/ws", hostPort)
	log.Printf("Submission %s ready at %s", subID, endpoint)
	return d.pub.Publish(subID, endpoint)
}

// buildImage creates a Docker image from the local tarball file.
func (d *DockerDeployer) buildImage(ctx context.Context, localTarball, tag string, profile languageProfile) error {
	dockerfile := fmt.Sprintf(`FROM %s
WORKDIR /app
COPY submission.tar.gz .
RUN tar -xzf submission.tar.gz && rm submission.tar.gz
RUN %s
CMD ["/engine"]
`, profile.BaseImage, profile.BuildCmd)

	buf := new(bytes.Buffer)
	tw := tar.NewWriter(buf)

	// Dockerfile
	if err := tw.WriteHeader(&tar.Header{
		Name: "Dockerfile",
		Size: int64(len(dockerfile)),
	}); err != nil {
		return err
	}
	if _, err := tw.Write([]byte(dockerfile)); err != nil {
		return err
	}

	// Submission tarball (local file)
	fileData, err := os.ReadFile(localTarball)
	if err != nil {
		return fmt.Errorf("reading local tarball: %w", err)
	}
	if err := tw.WriteHeader(&tar.Header{
		Name: "submission.tar.gz",
		Size: int64(len(fileData)),
	}); err != nil {
		return err
	}
	if _, err := tw.Write(fileData); err != nil {
		return err
	}

	if err := tw.Close(); err != nil {
		return err
	}

	resp, err := d.client.ImageBuild(ctx, bytes.NewReader(buf.Bytes()), types.ImageBuildOptions{
		Tags:       []string{tag},
		Dockerfile: "Dockerfile",
		Remove:     true,
	})
	if err != nil {
		return fmt.Errorf("image build failed: %w", err)
	}
	defer resp.Body.Close()

	body, _ := io.ReadAll(resp.Body)
	log.Printf("Build output: %s", string(body))
	return nil
}

// runContainer starts the built image and returns the random host port.
func (d *DockerDeployer) runContainer(ctx context.Context, imageTag, subID string) (int, error) {
	resp, err := d.client.ContainerCreate(ctx,
		&container.Config{
			Image: imageTag,
			Cmd:   []string{"/engine"},
			ExposedPorts: nat.PortSet{
				"8080/tcp": struct{}{},
			},
		},
		&container.HostConfig{
			Resources: container.Resources{
				Memory:   512 * 1024 * 1024, // 512 MB
				NanoCPUs: 1_000_000_000,     // 1 CPU
			},
			PortBindings: nat.PortMap{
				"8080/tcp": []nat.PortBinding{
					{HostPort: "0"}, // random port
				},
			},
			AutoRemove: true,
		},
		nil,
		nil,
		fmt.Sprintf("submission-%s", subID),
	)
	if err != nil {
		return 0, fmt.Errorf("container create: %w", err)
	}

	if err := d.client.ContainerStart(ctx, resp.ID, container.StartOptions{}); err != nil {
		return 0, fmt.Errorf("container start: %w", err)
	}

	time.Sleep(1 * time.Second) // wait for port assignment

	inspect, err := d.client.ContainerInspect(ctx, resp.ID)
	if err != nil {
		return 0, fmt.Errorf("inspect: %w", err)
	}

	for _, binding := range inspect.NetworkSettings.Ports["8080/tcp"] {
		if binding.HostPort != "" {
			port := 0
			fmt.Sscanf(binding.HostPort, "%d", &port)
			return port, nil
		}
	}
	return 0, fmt.Errorf("no host port mapped")
}

// ---------- MinIO helper ----------

func downloadFromMinio(objectKey string) (string, error) {
	endpoint := "localhost:9000"
	accessKey := "minioadmin"
	secretKey := "minioadmin"

	minioClient, err := minio.New(endpoint, &minio.Options{
		Creds:  credentials.NewStaticV4(accessKey, secretKey, ""),
		Secure: false,
	})
	if err != nil {
		return "", fmt.Errorf("creating MinIO client: %w", err)
	}

	bucket := "submissions"
	tmpFile := filepath.Join(os.TempDir(), filepath.Base(objectKey))

	err = minioClient.FGetObject(context.Background(), bucket, objectKey, tmpFile, minio.GetObjectOptions{})
	if err != nil {
		return "", fmt.Errorf("downloading object %s: %w", objectKey, err)
	}

	return tmpFile, nil
}