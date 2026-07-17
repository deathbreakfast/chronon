# AWS provision checklist

Region: `us-west-2` (override with `AWS_REGION`).

## 1. Artifacts

Campaign reports stay in-repo under `profiling/chronon-bench/reports/`. Generated fleet state and resume markers are gitignored (`instances.env`, `fleet-state.json`, `infra/aws/chronon/.state/`). No S3 bucket is required.

## 2. VPC (example CIDR `10.42.0.0/16`)

- Create VPC, public subnet, IGW, route table
- Security group `chronon-data-sg`: ingress 5432/6379 from `chronon-app-sg`
- Security group `chronon-app-sg`: egress all; SSM ingress

## 3. IAM instance profile

- Role with `AmazonSSMManagedInstanceCore`
- Attach to all EC2 instances

## 4. Launch instances

| Name | Type | User-data |
|------|------|-----------|
| chronon-data-services | t3.medium | `user-data/data.sh` |
| chronon-e2e-runner | t3.medium | `user-data/e2e.sh` |

Tag: `Project=chronon`

## 5. Wire env on e2e-runner

Copy `env/e2e-runner.env`, replace `DATA_PRIVATE_IP` with data-services private IP.

## 6. Clone + test

```bash
git clone <repo> ~/chronon && cd ~/chronon
source /opt/chronon/e2e.env
./infra/aws/chronon/run-e2e-aws.sh
```

From an operator machine:

```bash
export CHRONON_E2E_HOST=<e2e-runner public host>
export CHRONON_DATA_IP=<data-services private IP>
export CHRONON_SSH_KEY=$HOME/.ssh/<key>.pem
./infra/aws/chronon/deploy-and-run-e2e.sh
```

## 7. Distributed tier (optional)

Launch `chronon-coordinator`, `chronon-worker-a`, `chronon-worker-b` (t3.medium each).

On each worker:

```bash
cargo run -p uf-chronon --example worker_daemon --features postgres,redis
```

On coordinator:

```bash
cargo run -p uf-chronon --example coordinator_daemon --features postgres,redis
```

On e2e-runner:

```bash
CHRONON_DISTRIBUTED_MODE=remote ./infra/aws/chronon/run-distributed-smoke.sh
```
