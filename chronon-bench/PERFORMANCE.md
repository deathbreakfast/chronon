# Chronon performance

Measured on AWS (`t3.medium`, `c6i.large`, and multi-host fleets). Chronon is a scheduled job runtime: apps register cron and run-once work; the scheduler ticks, claims due runs, and workers execute them. Storage adapters (`mem`, `sqlite`, `postgres`, `postgres-redis`) change durability and claim throughput. Full ladders come from AWS campaign runs.

## Scheduler overhead

Empty-tick p95 is **~255 ms** on `t3.medium` when the configured tick interval is the bound. Due-job queries at about 1k registered jobs stay around **~5.3 ms** p95 on Postgres+Redis.

## Claim and drain capacity

Postgres-only claim throughput plateaus around **~296/s** at moderate queue depth; hybrid Postgres+Redis reaches about **1k claims/s** on the same host class. Multi-cell fleets scale near-linearly at **~470/s per cell** (**7,742/s @ 16 cells** on `c6i.large`). Large aggregates need many cells because each cell is bound by the durable claim path, not by Redis alone.

## Resilience and execution

Failover reclaim p95 was **331 ms** on the measured AWS layouts (within a 500 ms budget). Coordinator–worker deployment adds a small enqueue-to-run tax versus fully embedded scheduling.

BM-CH5 remains the sequential lifecycle-overhead baseline: it waits for one Success at a time, so the published runs/s figure is tick-and-wait cost, not burst capacity. `bm-ch-embed-burst` is the embedded-capacity result: 500 due jobs plus a colliding recurring cohort, workers draining in parallel. Publish that row only from an AWS sqlite report with `status=ok`. `bm-ch-fleet-burst` is the same workload on coordinator-worker hosts against Postgres+Redis; the scale axis is worker hosts (1/2/4). Publish that row only from an AWS postgres-redis report with `status=ok`.

## How to read these results

Use AWS hardware profiles for capacity planning. Local harness labels are not decision-grade for production fleets.
