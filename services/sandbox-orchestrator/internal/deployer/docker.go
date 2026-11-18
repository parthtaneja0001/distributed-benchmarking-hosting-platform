package deployer

import (
	"archive/tar"
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/container"
	"github.com/docker/docker/api/types/image"
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
	status *StatusWriter
	// How long a sandbox may live after it becomes ready (load test + slack).
	lifetime time.Duration
}

func NewDockerDeployer(pub *SandboxReadyPublisher) (*DockerDeployer, error) {
	cli, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation())
	if err != nil {
		return nil, fmt.Errorf("failed to create Docker client: %w", err)
	}
	lifetime := 120 * time.Second
	if v, err := strconv.Atoi(os.Getenv("SANDBOX_LIFETIME_SECS")); err == nil && v > 0 {
		lifetime = time.Duration(v) * time.Second
	}
	return &DockerDeployer{client: cli, pub: pub, status: NewStatusWriter(), lifetime: lifetime}, nil
}

// Deploy builds and runs the submission, then publishes sandbox.ready.
// Failures are recorded on the submission's status and not retried (a broken
// submission would otherwise block the queue forever).
func (d *DockerDeployer) Deploy(event events.SubmissionCreated) error {
	if err := d.deploy(event); err != nil {
		log.Printf("Submission %s failed: %v", event.ID, err)
		d.status.Set(event.ID, "failed", err.Error())
	}
	return nil
}

func (d *DockerDeployer) deploy(event events.SubmissionCreated) error {
	ctx := context.Background()
	subID := event.ID
	objectKey := event.ObjectKey // MinIO object key, e.g. <uuid>/submission.tar.gz
	lang := event.Language

	profile, ok := profiles[lang]
	if !ok {
		profile = profiles["unknown"]
	}
	log.Printf("Deploying submission %s (lang: %s) object key: %s", subID, lang, objectKey)
	d.status.Set(subID, "building", "")

	// 1. Download the tarball from MinIO
	localTarball, err := downloadFromMinio(objectKey)
	if err != nil {
		return fmt.Errorf("download from MinIO failed: %w", err)
	}
	defer os.Remove(localTarball)

	// 2. Build the container image
	tag := fmt.Sprintf("submission-%s:latest", subID)
	buildCtx, cancelBuild := context.WithTimeout(ctx, 10*time.Minute)
	defer cancelBuild()
	if err := d.buildImage(buildCtx, localTarball, tag, profile); err != nil {
		return fmt.Errorf("build failed: %w", err)
	}

	// 3. Run the container
	containerID, hostPort, err := d.runContainer(ctx, tag, subID)
	if err != nil {
		d.removeImage(tag)
		return fmt.Errorf("run failed: %w", err)
	}
	// Tear the sandbox down after its test window, whatever happens next.
	time.AfterFunc(d.lifetime, func() { d.cleanup(containerID, tag, subID) })

	// 4. Wait until the engine accepts connections before sending load.
	if err := waitForPort(hostPort, 30*time.Second); err != nil {
		return fmt.Errorf("engine did not start listening on :8080 within 30s")
	}

	endpoint := fmt.Sprintf("ws://localhost:%d/ws", hostPort)
	log.Printf("Submission %s ready at %s", subID, endpoint)
	if err := d.pub.Publish(subID, endpoint); err != nil {
		return err
	}
	d.status.Set(subID, "running", "")

	// Benchmarks run one at a time: hold the queue until this run's load test
	// window is over, so concurrent runs don't skew each other's numbers.
	window := 35 * time.Second
	if v, err := strconv.Atoi(os.Getenv("BENCHMARK_WINDOW_SECS")); err == nil && v > 0 {
		window = time.Duration(v) * time.Second
	}
	time.Sleep(window)
	return nil
}

func waitForPort(port int, timeout time.Duration) error {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		conn, err := net.DialTimeout("tcp", fmt.Sprintf("127.0.0.1:%d", port), time.Second)
		if err == nil {
			conn.Close()
			return nil
		}
		time.Sleep(500 * time.Millisecond)
	}
	return fmt.Errorf("timeout")
}

func (d *DockerDeployer) cleanup(containerID, tag, subID string) {
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	timeout := 5
	if err := d.client.ContainerStop(ctx, containerID, container.StopOptions{Timeout: &timeout}); err != nil {
		log.Printf("stop sandbox %s: %v", subID, err)
	}
	d.removeImage(tag)
	log.Printf("Sandbox %s cleaned up", subID)
}

func (d *DockerDeployer) removeImage(tag string) {
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	// Wait briefly for AutoRemove to delete the container that uses the image.
	for i := 0; i < 10; i++ {
		if _, err := d.client.ImageRemove(ctx, tag, image.RemoveOptions{Force: true, PruneChildren: true}); err == nil {
			return
		}
		time.Sleep(time.Second)
	}
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

	// The build API returns 200 even when a step fails; the failure is in the
	// JSON message stream, so it must be inspected.
	var lastLines []string
	scanner := bufio.NewScanner(resp.Body)
	scanner.Buffer(make([]byte, 0, 64*1024), 4*1024*1024)
	for scanner.Scan() {
		var msg struct {
			Stream      string `json:"stream"`
			Error       string `json:"error"`
			ErrorDetail struct {
				Message string `json:"message"`
			} `json:"errorDetail"`
		}
		if json.Unmarshal(scanner.Bytes(), &msg) != nil {
			continue
		}
		if line := strings.TrimSpace(msg.Stream); line != "" {
			lastLines = append(lastLines, line)
			if len(lastLines) > 8 {
				lastLines = lastLines[1:]
			}
		}
		if msg.Error != "" {
			detail := msg.ErrorDetail.Message
			if detail == "" {
				detail = msg.Error
			}
			return fmt.Errorf("%s | %s", detail, strings.Join(lastLines, " | "))
		}
	}
	if err := scanner.Err(); err != nil {
		return fmt.Errorf("reading build output: %w", err)
	}
	log.Printf("Built image %s", tag)
	return nil
}

// runContainer starts the built image and returns its id and random host port.
func (d *DockerDeployer) runContainer(ctx context.Context, imageTag, subID string) (string, int, error) {
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
				Memory:     512 * 1024 * 1024, // 512 MB
				MemorySwap: 512 * 1024 * 1024, // no extra swap
				NanoCPUs:   1_000_000_000,     // 1 CPU
				PidsLimit:  &pidsLimit,        // no fork bombs
			},
			// Untrusted code: drop all Linux capabilities and privilege escalation.
			CapDrop:     []string{"ALL"},
			SecurityOpt: []string{"no-new-privileges"},
			PortBindings: nat.PortMap{
				"8080/tcp": []nat.PortBinding{
					// Bind on loopback only so the engine isn't reachable from the LAN.
					{HostIP: "127.0.0.1", HostPort: "0"},
				},
			},
			AutoRemove: true,
		},
		nil,
		nil,
		fmt.Sprintf("submission-%s", subID),
	)
	if err != nil {
		return "", 0, fmt.Errorf("container create: %w", err)
	}

	if err := d.client.ContainerStart(ctx, resp.ID, container.StartOptions{}); err != nil {
		return "", 0, fmt.Errorf("container start: %w", err)
	}

	time.Sleep(1 * time.Second) // wait for port assignment

	inspect, err := d.client.ContainerInspect(ctx, resp.ID)
	if err != nil {
		return "", 0, fmt.Errorf("inspect: %w", err)
	}
	if inspect.State != nil && !inspect.State.Running {
		return "", 0, fmt.Errorf("engine exited immediately (exit code %d)", inspect.State.ExitCode)
	}

	for _, binding := range inspect.NetworkSettings.Ports["8080/tcp"] {
		if binding.HostPort != "" {
			port := 0
			fmt.Sscanf(binding.HostPort, "%d", &port)
			return resp.ID, port, nil
		}
	}
	return "", 0, fmt.Errorf("no host port mapped")
}

var pidsLimit int64 = 256

// ---------- MinIO helper ----------

func downloadFromMinio(objectKey string) (string, error) {
	endpoint := os.Getenv("MINIO_ADDR")
	if endpoint == "" {
		endpoint = "localhost:9000"
	}
	accessKey := "minioadmin"
	secretKey := "minioadmin"

	minioClient, err := minio.New(endpoint, &minio.Options{
		Creds:  credentials.NewStaticV4(accessKey, secretKey, ""),
		Secure: false,
		// Path-style (host/bucket/key), same as the submission service uses;
		// virtual-host style 404s on local S3 servers.
		BucketLookup: minio.BucketLookupPath,
		// A fixed region skips the GetBucketLocation call, which some
		// S3-compatible servers (e.g. s3mock) don't implement.
		Region: "us-east-1",
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
