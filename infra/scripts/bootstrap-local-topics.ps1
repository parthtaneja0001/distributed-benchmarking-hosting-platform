param(
    [string]$ContainerName = "redpanda",
    [string[]]$Topics = @("submission.created", "sandbox.ready", "telemetry.raw")
)

$ErrorActionPreference = "Stop"

function Test-ContainerRunning {
    param([string]$Name)

    $status = docker inspect -f "{{.State.Running}}" $Name 2>$null
    return $status -eq "true"
}

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    throw "Docker CLI is required but was not found in PATH."
}

if (-not (Test-ContainerRunning -Name $ContainerName)) {
    throw "Container '$ContainerName' is not running. Start infra first with: docker compose up -d"
}

$existingTopics = docker exec $ContainerName rpk topic list | Select-Object -Skip 1 | ForEach-Object {
    ($_ -split "\s+")[0]
}

foreach ($topic in $Topics) {
    if ($existingTopics -contains $topic) {
        Write-Host "topic exists: $topic"
        continue
    }

    Write-Host "creating topic: $topic"
    docker exec $ContainerName rpk topic create $topic | Out-Host
}
