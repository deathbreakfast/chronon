# Chronon AWS fleet (E2E first, then benchmarks)

Isolated VPC + EC2 fleet for correctness E2E, then `chronon-bench` campaigns.

**Quota:** steady-state ≤10 vCPUs (16 vCPU account limit).

## Fleet layout

| Node | Instance | vCPU | Role |
|------|----------|------|------|
| `data-services` | `t3.medium` | 2 | Postgres 16 + Redis 7 |
| `e2e-runner` | `t3.medium` | 2 | `cargo test` driver |
| `coordinator` | `t3.medium` | 2 | `coordinator_daemon` (distributed tier) |
| `worker-a`, `worker-b` | `t3.medium` × 2 | 4 | `worker_daemon` |

Start with **2 nodes** (data + e2e-runner, 4 vCPUs) for matrix E2E; add coordinator/workers for multi-process smokes.

## Tags

```json
{"Project": "chronon", "Component": "e2e"}
```

## Required operator env

```bash
export CHRONON_E2E_HOST=<e2e-runner public DNS or IP>
export CHRONON_DATA_IP=<data-services private IP>   # when not using compose on the CI host
export CHRONON_SSH_KEY=$HOME/.ssh/<key>.pem
export CHRONON_KEY_NAME=chronon-bench-key             # AWS key pair name for provision scripts
```

Generated fleet files (`fleet-state.json`, `instances.env`, `instances.cells.env`, resume markers) are written under the tree and gitignored — see `.gitignore` and `infra/aws/chronon/.state/`.

## E2E / CI commands

On the e2e-runner:

```bash
source /opt/chronon/e2e.env   # CHRONON_POSTGRES_URL, CHRONON_REDIS_URL
cd chronon
./infra/aws/chronon/run-e2e-aws.sh
```

From an operator machine with the SSH key and required env vars:

```bash
./infra/aws/chronon/run-remote-ci.sh
./infra/aws/chronon/deploy-and-run-e2e.sh
```

## Benchmark commands (after E2E gate)

```bash
export CHRONON_BENCH_HARDWARE=aws-t3-medium
./chronon-bench/scripts/run-durable-floor.sh
# … see chronon-bench/EXPERIMENTS.md
```

## Artifacts

Decision-grade reports live under `profiling/chronon-bench/reports/` (commit JSON after campaigns). S3 is not required.

- **Baseline campaign:** 85 JSON files on `aws-t3.medium` (2026-07-09)
- **CH7-D hyperscale:** per-cell JSON + scaling curves (`scaling-curve-ch7-workers|pools|multibench|ch7d-fleet-*-aws-c6i-large.json`)

## Hyperscale scaling fleet (CH7 D0–D5)

Burst ≤16 vCPUs; teardown between phases. See [`scaling-fleet/`](scaling-fleet/).

| Phase | Nodes | vCPU | Purpose |
|-------|-------|------|---------|
| D0–D2 | 1× bench + data | 4–6 | W / K / data-tier curves |
| D3 bc=4 | 4× bench + data | 10 | Multibench aggregate |
| D4 Wn=4 | 1× bench + 4× worker + data | 12 | BM-CH7D production path |
| D5 T0–T7 | ladder (cells, split, cluster, …) | varies | 10k/s release-gate ladder |

```bash
./infra/aws/chronon/scaling-fleet/provision-scaling-fleet.sh d0
./infra/aws/chronon/scaling-fleet/scripts/run-ch7-full-campaign-aws.sh
./infra/aws/chronon/scaling-fleet/scripts/run-ch7-d5-full-ladder-aws.sh
./infra/aws/chronon/scaling-fleet/scripts/fetch-reports.sh
```

Interpretation and sizing: [`chronon-bench/PERFORMANCE_STUDY.md`](../../../chronon-bench/PERFORMANCE_STUDY.md). Experiment registry: [`chronon-bench/EXPERIMENTS.md`](../../../chronon-bench/EXPERIMENTS.md).

## Provision / teardown

- Checklist: [`provision.md`](provision.md)
- Teardown: [`teardown.md`](teardown.md) / `scaling-fleet/teardown-chronon-fleet.sh`
