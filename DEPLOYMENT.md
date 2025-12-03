# Production Deployment Guide for SpacePanda Core

## Overview

This guide covers production deployment of SpacePanda Core using Docker, Docker Compose, and Kubernetes.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Configuration](#configuration)
3. [Docker Deployment](#docker-deployment)
4. [Docker Compose Deployment](#docker-compose-deployment)
5. [Kubernetes Deployment](#kubernetes-deployment)
6. [Monitoring and Observability](#monitoring-and-observability)
7. [Health Checks](#health-checks)
8. [Troubleshooting](#troubleshooting)

---

## Prerequisites

- Docker 20.10+
- Docker Compose 2.0+ (for local deployment)
- Kubernetes 1.24+ (for production cluster deployment)
- `kubectl` configured for your cluster
- Prometheus and Grafana (optional, for monitoring)

## Configuration

### Environment Variables

SpacePanda Core uses environment variables for configuration. All variables follow the pattern `SPACEPANDA_<SECTION>_<KEY>`.

**Server Configuration:**
```bash
SPACEPANDA_SERVER_BIND_ADDRESS=0.0.0.0:8080
SPACEPANDA_SERVER_MAX_CONNECTIONS=10000
SPACEPANDA_SERVER_ENABLE_TLS=false
```

**DHT Configuration:**
```bash
SPACEPANDA_DHT_BUCKET_SIZE=20
SPACEPANDA_DHT_REPLICATION_FACTOR=3
```

**Store Configuration:**
```bash
SPACEPANDA_STORE_DATA_DIR=/app/data
SPACEPANDA_STORE_ENABLE_WAL=true
```

**Logging Configuration:**
```bash
SPACEPANDA_LOG_LEVEL=info  # trace, debug, info, warn, error
SPACEPANDA_LOG_JSON=true   # Enable JSON structured logging
```

**Metrics Configuration:**
```bash
SPACEPANDA_METRICS_ENABLED=true
SPACEPANDA_METRICS_BIND_ADDRESS=0.0.0.0:9090
```

### Configuration File

Alternatively, use a TOML configuration file:

```bash
cp config/config.example.toml config/config.toml
# Edit config.toml with your settings
```

---

## Docker Deployment

### Building the Image

```bash
# Build from project root
docker build -t spacepanda/core:latest .

# Build with specific version tag
docker build -t spacepanda/core:1.0.0 .
```

### Running the Container

```bash
docker run -d \
  --name spacepanda-core \
  -p 8080:8080 \
  -p 9090:9090 \
  -e SPACEPANDA_LOG_LEVEL=info \
  -e SPACEPANDA_LOG_JSON=true \
  -v $(pwd)/data:/app/data \
  -v $(pwd)/config:/app/config:ro \
  spacepanda/core:latest
```

### Container Management

```bash
# View logs
docker logs -f spacepanda-core

# Check health
docker exec spacepanda-core curl http://localhost:8080/health

# Stop gracefully
docker stop spacepanda-core

# Remove container
docker rm spacepanda-core
```

---

## Docker Compose Deployment

### Quick Start

```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f

# Scale SpacePanda instances
docker-compose up -d --scale spacepanda=3

# Stop all services
docker-compose down

# Stop and remove volumes
docker-compose down -v
```

### Services Included

- **spacepanda**: Core application
- **prometheus**: Metrics collection (port 9091)
- **grafana**: Visualization dashboard (port 3000)

### Accessing Services

- SpacePanda: http://localhost:8080
- Prometheus: http://localhost:9091
- Grafana: http://localhost:3000 (admin/spacepanda)
- Metrics: http://localhost:9090/metrics

---

## Kubernetes Deployment

### Prerequisites

```bash
# Create namespace
kubectl create namespace spacepanda

# Or apply the full manifest (includes namespace)
kubectl apply -f deploy/kubernetes.yaml
```

### Deployment Steps

1. **Create ConfigMap:**
```bash
kubectl create configmap spacepanda-config \
  --from-file=config/config.toml \
  -n spacepanda
```

2. **Create PersistentVolumeClaim:**
```bash
# Already included in kubernetes.yaml
kubectl get pvc -n spacepanda
```

3. **Deploy Application:**
```bash
kubectl apply -f deploy/kubernetes.yaml
```

4. **Verify Deployment:**
```bash
# Check pods
kubectl get pods -n spacepanda

# Check services
kubectl get svc -n spacepanda

# View logs
kubectl logs -f deployment/spacepanda-core -n spacepanda
```

### Scaling

```bash
# Manual scaling
kubectl scale deployment spacepanda-core --replicas=5 -n spacepanda

# Auto-scaling is configured via HorizontalPodAutoscaler
kubectl get hpa -n spacepanda
```

### Updating

```bash
# Update image
kubectl set image deployment/spacepanda-core \
  spacepanda-core=spacepanda/core:1.1.0 \
  -n spacepanda

# Rollout status
kubectl rollout status deployment/spacepanda-core -n spacepanda

# Rollback if needed
kubectl rollout undo deployment/spacepanda-core -n spacepanda
```

---

## Monitoring and Observability

### Prometheus Metrics

SpacePanda exposes Prometheus metrics on port 9090:

**Available Metrics:**

- **CRDT Metrics:**
  - `crdt.or_set.add` - ORSet add operations
  - `crdt.or_set.merge` - ORSet merge operations
  - `crdt.or_set.merge.duration_ms` - Merge duration

- **DHT Metrics:**
  - `dht.requests.total` - Total DHT requests
  - `dht.peers.active` - Active peer count
  - `dht.request.duration_ms` - Request latency

- **Store Metrics:**
  - `store.operations.total` - Total store ops
  - `store.size.bytes` - Store size
  - `store.tombstones.count` - Tombstone count

- **Network Metrics:**
  - `network.messages.sent` - Messages sent
  - `network.latency_ms` - Network latency

### Accessing Metrics

```bash
# Direct access
curl http://localhost:9090/metrics

# In Kubernetes
kubectl port-forward svc/spacepanda-metrics 9090:9090 -n spacepanda
curl http://localhost:9090/metrics
```

### Structured Logging

Enable JSON logging for production:

```bash
SPACEPANDA_LOG_JSON=true
```

Logs include trace IDs for distributed tracing:

```json
{
  "timestamp": "2025-12-03T10:30:45Z",
  "level": "INFO",
  "trace_id": "550e8400-e29b-41d4-a716-446655440000",
  "span_id": "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
  "operation": "crdt_merge",
  "duration_ms": 42
}
```

---

## Health Checks

SpacePanda provides multiple health check endpoints:

### Liveness Probe

**Endpoint:** `GET /health/live`

Checks if the process is running. Returns 200 if alive.

```bash
curl http://localhost:8080/health/live
```

### Readiness Probe

**Endpoint:** `GET /health/ready`

Checks if the service can accept traffic. Returns 200 if ready, 503 if not.

```bash
curl http://localhost:8080/health/ready
```

**Response:**
```json
{
  "status": "healthy",
  "timestamp": "2025-12-03T10:30:45Z",
  "components": [
    {
      "name": "dht",
      "status": "healthy",
      "message": null,
      "last_check": "2025-12-03T10:30:45Z"
    },
    {
      "name": "store",
      "status": "healthy",
      "message": null,
      "last_check": "2025-12-03T10:30:45Z"
    }
  ],
  "version": "1.0.0",
  "uptime_seconds": 3600
}
```

### Overall Health

**Endpoint:** `GET /health`

Returns comprehensive health status.

---

## Troubleshooting

### Common Issues

#### 1. Container Won't Start

```bash
# Check logs
docker logs spacepanda-core

# Check configuration
docker exec spacepanda-core cat /app/config/config.toml
```

#### 2. High Memory Usage

```bash
# Check metrics
curl http://localhost:9090/metrics | grep memory

# Adjust limits in docker-compose.yml or kubernetes.yaml
```

#### 3. Connection Issues

```bash
# Verify ports are exposed
docker ps
kubectl get svc -n spacepanda

# Check firewall rules
sudo iptables -L
```

#### 4. Slow Performance

```bash
# Check metrics
curl http://localhost:9090/metrics | grep duration

# Review benchmarks
cd spacepanda-core
cargo bench
```

### Debug Mode

Enable debug logging:

```bash
SPACEPANDA_LOG_LEVEL=debug docker-compose up
```

### Support

For issues:
1. Check logs: `docker logs` or `kubectl logs`
2. Verify health: `/health/ready` endpoint
3. Review metrics: `/metrics` endpoint
4. Consult documentation: `docs/`

---

## Security Considerations

### Production Checklist

- [ ] Enable TLS with valid certificates
- [ ] Use secrets management (Kubernetes Secrets, Vault)
- [ ] Run as non-root user (default in Dockerfile)
- [ ] Apply resource limits (CPU, memory)
- [ ] Enable network policies (Kubernetes)
- [ ] Regular security updates
- [ ] Audit logs enabled
- [ ] Rate limiting configured
- [ ] Circuit breaker enabled

### TLS Configuration

```toml
[server]
enable_tls = true
tls_cert_path = "/app/config/cert.pem"
tls_key_path = "/app/config/key.pem"
```

---

## Performance Tuning

### Resource Allocation

**Recommended:**
- CPU: 1-2 cores
- Memory: 1-2 GB
- Storage: 10+ GB SSD

**High Load:**
- CPU: 4+ cores
- Memory: 4+ GB
- Storage: 50+ GB NVMe SSD

### Optimization Flags

See `ORSET_OPTIMIZATION_REPORT.md` for performance baseline:

- ORSet merge: 4.8-5.3 Melem/s
- DHT lookup: 5.4 Melem/s
- Crypto: 640 MB/s ChaCha20Poly1305

---

## Backup and Recovery

### Data Backup

```bash
# Docker
docker cp spacepanda-core:/app/data ./backup/

# Kubernetes
kubectl cp spacepanda/spacepanda-core-xxx:/app/data ./backup/
```

### Restore

```bash
# Docker
docker cp ./backup/ spacepanda-core:/app/data

# Kubernetes
kubectl cp ./backup/ spacepanda/spacepanda-core-xxx:/app/data
```

---

## License

SpacePanda Core - Production Deployment Guide
Copyright © 2025 SpacePanda Team
