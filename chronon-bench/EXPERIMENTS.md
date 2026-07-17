# Chronon benchmark experiment registry

Pre-registered experiment IDs, dimension matrix, sweep phases, results log, and runner commands.

**Methodology and interpretation:** [`PERFORMANCE_STUDY.md`](PERFORMANCE_STUDY.md)

This registry defines **BM-CH\*** (Chronon layer) and **BM-CHL\*** (sustained tick load). Decision-grade hardware is AWS-only.

---

## Validation status

| Tier | Storage | Status | Authoritative hardware |
|------|---------|--------|------------------------|
| **Scheduler floor** | mem | **Measured** | `aws-t3.medium` |
| **Scheduler floor** | sqlite | **Measured** | `aws-t3.medium` |
| **Scheduler floor** | postgres | **Measured** | `aws-t3.medium` |
| **Scheduler floor** | postgres-redis | **Measured** | `aws-t3.medium` |
| **Claim capacity** | postgres | **Measured** | `aws-t3.medium` (Tier 3 proxy) |
| **Claim capacity** | postgres-redis | **Measured** (primary gate) | `aws-t3.medium` (Tier 3 proxy) |
| **Hyperscale CH7-D** | postgres-redis | **Measured** | 4× `aws-c6i.large` (2026-07-11 D3 @ Q=100k) |
| **10k/s release gate** | postgres-redis | **Measured — gate not met** (peak 7,742/s @ 16 cells) | 16× `aws-c6i.large` + 16 cells (D5 T5, 2026-07-12) |
| **Sustained tick** | all | **Measured** | `aws-t3.medium` |
| **Resilience** | postgres, postgres-redis | **Measured** | `aws-t3.medium` |
| **Execution path** | all | **Measured** | `aws-t3.medium` |

---

## Authoritative test environment

Decision-grade numbers use the profiles below. Do not mix `local` smoke with AWS in-VPC runs.

### Tier 1/2 (`aws-t3.medium`)

| Field | Value |
|-------|-------|
| **Hardware** | `aws-t3.medium` (2 vCPU, 4 GiB) |
| **Topology** | `isolated-lab`, deployment `embedded`, telemetry `off` |
| **Production env** | `CHRONON_TICK_INTERVAL_MS=250`, `CHRONON_NUM_PARTITIONS=16`, `CHRONON_TICK_BATCH_LIMIT=500` |
| **Postgres** | Colocated Docker `postgres:16-alpine` or dedicated `t3.medium` |
| **Redis** | Dedicated `t3.medium` Redis 7 (`postgres-redis` only) |
| **Reports** | `profiling/chronon-bench/reports/bm-ch*-*-aws-t3-medium.json` |

### Tier 3 capacity (`aws-c6i.large`)

| Field | Value |
|-------|-------|
| **Bench** | 1× `c6i.large` + `t3.medium` Postgres + Redis |
| **CH7 primary row** | prefill 10k, W ∈ {8, 16, 32, 64}, pool `general` |
| **CHL** | BM-CHL2–3 (1k–10k due jobs/tick) |
| **Reports** | `profiling/chronon-bench/reports/scaling-curve-ch7-*-aws-c6i-large.json` |

### Harness smoke (non-authoritative)

CI/method checks may use `--hardware local`. Never mix those labels into decision-grade tables.

---

## Dimensions

| Dimension | Values | Notes |
|-----------|--------|-------|
| **Storage** | `mem`, `sqlite`, `postgres`, `postgres-redis` | `mem` → `chronon-backend-mem` |
| **Deployment** | `embedded`, `coordinator-worker`, `remote-client` | Builder shape |
| **Topology** | `isolated-lab`, `monolith-embedded`, `split-chronon-server`, `remote-coordinator` | |
| **Telemetry** | `off`, `console` | |
| **Hardware** | `aws-t3.medium`, `aws-c6i.large`, … | Decision-grade via `--hardware` / `CHRONON_BENCH_HARDWARE` |

---

## Experiment taxonomy

Three tracks — **do not mix metrics**:

| Track | IDs | Primary metric | Use |
|-------|-----|----------------|-----|
| **Tick path** | BM-CH0, CH1, CH3, CH4, BM-CHL* | tick/query p50/p95/p99 ms | Scheduler overhead |
| **Execution path** | BM-CH5, CH6 | runs/s, enqueue-to-run ms | Script + deployment tax |
| **Claim capacity** | BM-CH7, BM-CH7D | `claim_ops_per_sec` | Worker claim / production drain ceiling |

---

## Chronon layer experiment log

| ID | Workload | Primary metric | Pass criteria | Results |
|----|----------|----------------|---------------|---------|
| **BM-CH0** | Scheduler tick (empty due set) | tick wall p50/p95 | Flat vs tick index @ 1k ticks | aws-t3.medium PASS (p95 ~253–256 ms) |
| **BM-CH1** | Due-job query | query p50/p95 vs J | Sub-linear or documented bound | aws-t3.medium PASS |
| **BM-CH2** | Cron evaluation | evals/s | ≥ croner throughput | Method PASS (~396k vs 187k) |
| **BM-CH3** | Partition reassignment churn | reassignment p95, tick delay | Tick delay ≤ 2× baseline | aws-t3.medium PASS |
| **BM-CH4** | Leader failover | time-to-first-tick | ≤ 2× tick interval | aws-t3.medium PASS (331 ms p95) |
| **BM-CH5** | Noop script execution | runs/s vs tokio baseline | Document overhead | aws-t3.medium PASS (~3.6/s) |
| **BM-CH6** | Embedded vs coordinator-worker | enqueue-to-run p95 delta | Documented budget | aws-t3.medium PASS (+32 ms) |
| **BM-CH7** | Worker claim throughput (Track A) | `claim_ops_per_sec` vs W | err < 0.1%; hybrid gate | aws-t3.medium PASS (~1k/s); CH7-D0/D3 + D5 on `aws-c6i-large` |
| **BM-CH7D** | Production worker drain (Track B) | `claim_ops_per_sec` vs Wn | drain completes | aws-c6i-large PASS (~630/s single cell, flat across Wn; D5 T7) |
| **BM-CHL0** | Sustained 10 due jobs/tick | p99, error rate | err < 0.1% | aws-t3.medium PASS |
| **BM-CHL1** | Sustained 100 due jobs/tick | p99, error rate | err < 0.1% | aws-t3.medium PASS |
| **BM-CHL2** | Sustained 1k due jobs/tick | p99, error rate | err < 0.1% | aws-t3.medium PASS |
| **BM-CHL3** | Sustained 10k due jobs/tick | p99, error rate | err < 0.1% | aws-t3.medium PASS |

---

## Parameter sweep phases

Documented as sub-phases on existing experiment IDs.

| Phase | Knob | Values | Experiments | Isolates |
|-------|------|--------|-------------|----------|
| **S0** | Worker count W | {8, 16, 32, 64} | BM-CH7 (`--worker-count`) | Claim concurrency |
| **S1** | Job count J | {1k, 10k, 100k} | BM-CH1 (`--jobs`) | Due-query index cost |
| **S2** | Due jobs/tick D | {10, 100, 1k, 10k} | BM-CHL0–3 | Tick sustain ceiling |
| **S3** | Partition count P | {4, 16, 64} | BM-CH1, BM-CH3 (`--partitions`) | Partition spread |
| **S4** | Prefill Q | {1k, 10k, 100k} | BM-CH7 (`--prefill`) | Queue depth at claim |
| **S5** | Bench clients bc | {1, 2, 4} | BM-CH7 (`--bench-client-count`) | Multibench embed fleet |

### CH7-D hyperscale ladder

Two tracks — do not mix in one curve:

| Phase | Knob | Q default | Script | Curve |
|-------|------|-----------|--------|-------|
| **D0** | Worker W | 10k | `run-ch7-d0-worker-sweep.sh` | `ch7-worker-curve` |
| **D1** | Pools K | 10k | `run-ch7-pool-sweep.sh` | `ch7-pool-curve` |
| **D2** | Data tier N | 10k | `run-ch7-d2-aws.sh` | `ch7-data-curve` |
| **D3** | Bench clients bc | 100k | `run-ch7-multibench-sweep.sh` | `ch7-multibench-curve` |
| **D4** | Worker hosts Wn | 100k | `run-ch7d-fleet-sweep.sh` | `ch7d-fleet-curve` |

AWS orchestration: [`infra/aws/chronon/scaling-fleet/`](../infra/aws/chronon/scaling-fleet/).

```bash
# Local multibench smoke (bc=2, mem)
./chronon-bench/scripts/run-ch7-multibench-smoke.sh

# AWS full campaign D0→D4
./infra/aws/chronon/scaling-fleet/scripts/run-ch7-full-campaign-aws.sh

# Aggregate multibench cell
cargo run -p chronon-bench -- aggregate \
  --storage postgres-redis --hardware aws-c6i-large \
  --reports-dir profiling/chronon-bench/reports \
  --cell-prefix bm-ch7-bc2-w8-q100000
```
| **S6** | Deployment | embedded, coordinator-worker | BM-CH5, CH6 | Process split tax |
| **S7** | Telemetry | off, console | BM-CH0 | Sink overhead |

**Suggested sweep order per backend:**

```
mem:           S2 (CHL) → S1 (CH1) → S6 (CH6) → CH0/CH2/CH5 baseline
sqlite:        S1 → S2 (CHL0–1) → CH0
postgres:        S1 → S2 → S0 (CH7) → CH3/CH4
postgres-redis:  S0 (CH7 primary) → S4 → A/B vs postgres S0 → S2 (CHL2–3)
```

---

## Matrix slices

| Slice | Experiments | Storage | Purpose |
|-------|-------------|---------|---------|
| `adapter-floor` | BM-CH0, BM-CH1, BM-CH2 | mem | CI/dev ceiling |
| `durable-floor` | BM-CH0, BM-CH1 | sqlite, postgres, postgres-redis | Storage tax |
| `claim-capacity` | BM-CH7 | postgres, postgres-redis | Hybrid gate |
| `scheduler-sustain` | BM-CHL0–3 | all | Due jobs/tick ladder |
| `execution-path` | BM-CH5, BM-CH6 | all | E2E runs + deployment |
| `resilience` | BM-CH3, BM-CH4 | postgres, postgres-redis | Failover |
| `telemetry-tax` | BM-CH0 | mem | Console overhead |
| `cost-tier` | BM-CHL1 | mem vs postgres | TCO anchor |

```bash
cargo run -p chronon-bench -- matrix --slice durable-floor --storage postgres \
  --hardware aws-t3-medium --reports-dir profiling/chronon-bench/reports
```

---

## Question coverage matrix

| RQ | Question | Primary experiments |
|----|----------|---------------------|
| RQ1 | Tick latency floor | BM-CH0 |
| RQ2 | Due-query scaling | BM-CH1 S1, S3 |
| RQ3 | Cron eval tax | BM-CH2 |
| RQ4 | Partition churn | BM-CH3 |
| RQ5 | Leader failover | BM-CH4 |
| RQ6 | Script execution overhead | BM-CH5 |
| RQ7 | Deployment tax | BM-CH6 S6 |
| RQ8 | Claim throughput | BM-CH7 S0, S4 |
| RQ9 | Sustained tick ceiling | BM-CHL S2 |
| RQ10 | Hybrid ROI | BM-CH7 postgres vs postgres-redis |
| RQ11 | Multibench embed scaling | BM-CH7 D3 |
| RQ12 | Pool / data-tier knees | BM-CH7 D1, D2 |
| RQ13 | Production worker drain tax | BM-CH7D D4 |
| RQ14 | Hyperscale host projection | D0–D4 curves |

---

## Harness

`chronon-bench` provides:

| Subcommand | Purpose |
|------------|---------|
| `experiments` | List BM-CH* / BM-CHL* IDs |
| `run` | Single experiment with sweep knobs |
| `matrix` | Run a named slice × storage backend |
| `scaling-curve` | Project JSON reports into sweep curves |
| `aggregate` | Sum multibench BM-CH7 per-client reports |

**Scaling curve kinds:** `ch7-worker-curve`, `ch7-pool-curve`, `ch7-data-curve`, `ch7-multibench-curve`, `ch7d-fleet-curve`, `ch1-job-curve`, `chl-sustain-curve`

**Sweep knobs on `run`:**

| Flag | Env | Sweep |
|------|-----|-------|
| `--worker-count` | `CHRONON_BENCH_WORKER_COUNT` | S0 / D0 |
| `--partitions` | `CHRONON_BENCH_PARTITIONS` | S3 |
| `--prefill` | `CHRONON_BENCH_PREFILL` | S4 / D3–D4 |
| `--bench-client-index` | `CHRONON_BENCH_CLIENT_INDEX` | S5 / D3 |
| `--bench-client-count` | `CHRONON_BENCH_CLIENT_COUNT` | S5 / D3 |
| `--pool-count` | `CHRONON_BENCH_POOL_COUNT` | D1 |
| `--pool-layout` | `CHRONON_BENCH_POOL_LAYOUT` | D1 (`shared` / `distinct`) |
| `--worker-hosts` | `CHRONON_BENCH_WORKER_HOSTS` | D4 (BM-CH7D) |
| `--storage-topology` | `CHRONON_BENCH_STORAGE_TOPOLOGY` | D2 |
| `--jobs` | — | S1 / CHL seed |
| `--ops` | — | Iteration count |
| `--hardware` | `CHRONON_BENCH_HARDWARE` | Report tag |

Precedence: experiment defaults → env → CLI flags.

---

## Environment

| Variable | Effect |
|----------|--------|
| `CHRONON_TICK_INTERVAL_MS` | Scheduler tick period (CH0, CH4) |
| `CHRONON_NUM_PARTITIONS` | Partition count (overridden by `--partitions` in bench) |
| `CHRONON_TICK_BATCH_LIMIT` | Max due jobs enqueued per tick |
| `CHRONON_PARTITION_LEASE_TTL_S` | Partition lease TTL (CH3) |
| `CHRONON_RUN_LEASE_TTL_S` | Run claim lease (CH7) |
| `CHRONON_WORKER_CONCURRENCY` | Embedded worker pool size |
| `CHRONON_POSTGRES_URL` / `CHRONON_TEST_POSTGRES_URL` | Postgres backend |
| `CHRONON_REDIS_URL` / `CHRONON_TEST_REDIS_URL` | Redis hybrid backend |
| `CHRONON_BENCH_HARDWARE` | Report hardware slug |
| `CHRONON_BENCH_WORKER_COUNT` | CH7 worker sweep |
| `CHRONON_BENCH_PARTITIONS` | CH1/CH3 partition sweep |
| `CHRONON_BENCH_PREFILL` | CH7 prefill count |
| `CHRONON_BENCH_CLIENT_INDEX` | D3 multibench client index (0 = prefill) |
| `CHRONON_BENCH_CLIENT_COUNT` | S5 / D3 multibench client count |
| `CHRONON_BENCH_POOL_COUNT` | D1 pool shard count |
| `CHRONON_BENCH_POOL_LAYOUT` | D1 `shared` or `distinct` |
| `CHRONON_BENCH_WORKER_HOSTS` | D4 worker daemon host count |
| `CHRONON_BENCH_STORAGE_TOPOLOGY` | D2 colocated vs split label |
| `CHRONON_BENCH_CENTRAL_PREFILL` | Client 0 only prefill in multibench |
| `CHRONON_BENCH_DRAIN_ONLY` | Skip prefill (clients 1..bc-1) |
| `CHRONON_CH7_PIN_WORKER_POOLS` | Pin workers to pool shards |

---

## Run commands

```bash
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-chronon-bench

# List experiments
cargo run -p chronon-bench -- experiments

# Single experiment
cargo run -p chronon-bench -- run \
  --experiment bm-ch0 \
  --storage mem \
  --deployment embedded \
  --telemetry off \
  --ops 1000 \
  --hardware aws-t3.medium

# CH7 worker sweep (postgres-redis)
cargo run -p chronon-bench -- run \
  --experiment bm-ch7 \
  --storage postgres-redis \
  --worker-count 32 \
  --prefill 10000 \
  --hardware aws-c6i-large

# Matrix slice
cargo run -p chronon-bench -- matrix \
  --slice durable-floor \
  --storage sqlite \
  --hardware aws-t3-medium

# Scaling curve projection
cargo run -p chronon-bench -- scaling-curve ch7-worker-curve \
  --storage postgres-redis \
  --hardware aws-c6i-large \
  --reports-dir profiling/chronon-bench/reports
```

**Campaign scripts:** [`scripts/`](scripts/) — local sweep wrappers (no AWS provision).

**Reports:** `profiling/chronon-bench/reports/{id}-{matrix-slug}-{hardware}.json`

**Paper appendices:** [`PERFORMANCE_STUDY.md`](PERFORMANCE_STUDY.md) Appendix D (AWS baselines).

---

## Cloud results

| Profile | Instance | Date | Scheduler-floor | Max CHL | CH7 gate | Notes |
|---------|----------|------|-----------------|---------|----------|-------|
| `aws-t3.medium` | 2 vCPU, 4 GiB | 2026-07-09 | PASS (sqlite/postgres/postgres-redis) | CHL3 | PASS (W=32, prefill 10k) | 85 reports; E2E + bench on shared fleet |
| `aws-c6i-large` | 16× c6i.large + 16 cells | 2026-07-12 | — | — | **D5 T5 peak 7,742/s @ 16 cells** (10k gate not met); T6 batch no gain; T7 CH7D ~630/s flat | D5 ladder T0–T7 complete |
| `aws-c6i-large` | 4× c6i.large + data tier | 2026-07-11 | — | — | D0 **~1896/s** W=32; D3 W=1/host **708/s wall @ bc=4**; CH7D Wn=4 **610/s** | `scaling-curve-ch7-multibench-*-aws-c6i-large.json` |
| `aws-c6i-large` | 4× c6i.large bench | 2026-07-11 | — | — | Legacy D3 W=16: **~2,104/s** sum / **~1.3k/s wall** @ bc=2 | superseded by W=1/host campaign |

---

## D5 full ladder (T0–T7) — 10k/s release gate — **COMPLETE** (2026-07-12)

**Autorun:** `infra/aws/chronon/scaling-fleet/scripts/run-ch7-d5-full-ladder-aws.sh`

| Tier | Script | Knobs (primary row) | Result |
|------|--------|---------------------|--------|
| T0 | `run-ch7-t0-matrix.sh` | W/K/Q sweeps, PG pool, multibench bc∈{1,2,4} | single-pair ceiling ~1.9k/s @ W=32 |
| T1 | `run-ch7-t1-split.sh` | Real split r6g.large Redis + Postgres | split ≈ colocated |
| T2 | `run-ch7-t2-sized.sh` | r6g.xlarge sized pair | no material lift |
| T3 | `run-ch7-t3-cluster.sh` | Redis Cluster 3-node, hash-tag keys | Redis not the bottleneck |
| T4 | `run-ch7-t4-pg-scale.sh` | PgBouncer + PG pool ∈ {5..100} | PG UPDATE is the ceiling |
| T5 | `run-ch7-t5-multicell.sh` | N cells, W=1/host, **10k wall gate** | **peak 7,742/s @ 16 cells — gate not met** |
| T6 | `run-ch7-t6-batch.sh` | `CHRONON_CLAIM_BATCH` ∈ {1,4,8,16} | no gain (batch1 best 2,006/s single cell) |
| T7 | `run-ch7-t7-ch7d.sh` | Wn ∈ {1,2,4,8,16}, concurrency=1 | ~630/s flat (execute-path ceiling, single cell) |

### T5 multi-cell fleet scaling (postgres-redis, W=1/host, Q=100k)

| Cells N | Aggregate `fleet_wall_claim_ops_per_sec` | Per-cell |
|---------|------------------------------------------|----------|
| 1 | 442/s | 442/s |
| 2 | 932/s | 466/s |
| 4 | 1,809/s | 452/s |
| 8 | 3,794/s | 474/s |
| 16 | **7,742/s** | 484/s |

Near-linear per-cell (~470/s), so **~21 cells** project to 10k/s. Gate (≥10,000/s) not met at 16 cells; scaling is horizontal-clean, not blocked.

**Finding:** the binding constraint is the per-cell sync Postgres claim UPDATE (~470/s wall at W=1). Redis, instance size, and claim batching (T2/T3/T6) do not move it; only adding cells does (T5). Batched claim (T6) and the execute-tax drain (T7) stay at or below the single-cell ceiling.

**Reports:** `profiling/chronon-bench/reports/` (AWS-labeled JSON). Runtime resume markers (if any) live under gitignored `infra/aws/chronon/.state/`.

**Code knobs:** `CHRONON_PG_POOL_SIZE`, `CHRONON_CLAIM_BATCH`, `CHRONON_REDIS_CLUSTER_URLS`, `CHRONON_REDIS_HASH_TAGS`; per-cell URLs from generated `instances.cells.env`.

---

## Campaign infrastructure

| Component | Status |
|-----------|--------|
| `chronon-bench` CLI (`run`, `matrix`, `scaling-curve`, `aggregate`) | Implemented |
| `chronon-bench/scripts/*.sh` | Local + AWS sweep wrappers |
| `infra/aws/chronon/` fleet | Torn down after campaign (0 instances, us-west-2, 2026-07-12) |
| `infra/aws/chronon/scaling-fleet/` | **Implemented** — CH7-D0–D4 + **D5 T0–T7 ladder**; campaign **complete** (2026-07-12) |

Do not run rate-target experiments from a laptop over public DB URLs.
