#!/usr/bin/env bash
# Bootstrap self-hosted Redis Cluster (3 nodes) on existing bench/data hosts or new nodes.
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FLEET_STATE="$(cd "$SF/.." && pwd)/fleet-state.json"
REGION="${AWS_REGION:-us-west-2}"
# shellcheck disable=SC1091
source "$SF/lib/bench-fleet.sh"
# shellcheck disable=SC1091
source "$SF/instances.env"

SG_ID="$(jq -r '.security_group_id' "$FLEET_STATE")"
aws ec2 authorize-security-group-ingress --region "$REGION" \
  --group-id "$SG_ID" \
  --ip-permissions "IpProtocol=tcp,FromPort=7000,ToPort=7002,UserIdGroupPairs=[{GroupId=${SG_ID}}]" \
  2>/dev/null || true

SSH_KEY="$(bench_ssh_key)"
SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=30 -i "$SSH_KEY")
NODES="${CHRONON_REDIS_CLUSTER_NODES:-3}"
CLUSTER_PORTS=(7000 7001 7002)

DATA_HOST="ec2-user@${DATA_PUBLIC_IP:-${DATA_IP}}"

echo "bootstrapping Redis Cluster on ${DATA_HOST} (${NODES} nodes via docker)..."

# Skip recreate when cluster is already healthy (T3 resume / supervisor restart).
if ssh "${SSH_OPTS[@]}" "$DATA_HOST" bash -s <<'CHECK' 2>/dev/null; then
set -euo pipefail
docker exec chronon-redis-7000 redis-cli cluster info 2>/dev/null | grep -q cluster_state:ok
CHECK
  CLUSTER_URLS="redis://${DATA_IP}:7000,redis://${DATA_IP}:7001,redis://${DATA_IP}:7002"
  export CHRONON_REDIS_CLUSTER_URLS="$CLUSTER_URLS"
  export CHRONON_REDIS_HASH_TAGS=1
  grep -q '^CHRONON_REDIS_CLUSTER_URLS=' "$SF/instances.env" 2>/dev/null || {
    echo "CHRONON_REDIS_CLUSTER_URLS=${CLUSTER_URLS}" >> "$SF/instances.env"
    echo "CHRONON_REDIS_HASH_TAGS=1" >> "$SF/instances.env"
    echo "REDIS_CLUSTER_IP=${DATA_IP}" >> "$SF/instances.env"
  }
  echo "Redis Cluster already ok: ${CLUSTER_URLS}"
  exit 0
fi

ssh "${SSH_OPTS[@]}" "$DATA_HOST" bash -s <<REMOTE
set -euo pipefail
for p in 7000 7001 7002; do
  docker rm -f chronon-redis-\$p 2>/dev/null || true
  docker run -d --name chronon-redis-\$p --net host redis:7-alpine \
    redis-server --port \$p --cluster-enabled yes \
    --cluster-config-file nodes-\$p.conf --cluster-node-timeout 5000 \
    --appendonly no --save ""
done
sleep 5
HOST=\$(curl -sf --connect-timeout 2 -H "X-aws-ec2-metadata-token: \$(curl -sf -X PUT http://169.254.169.254/latest/api/token -H 'X-aws-ec2-metadata-token-ttl-seconds: 60')" \
  http://169.254.169.254/latest/meta-data/local-ipv4 2>/dev/null || true)
if [[ -z "\$HOST" ]]; then
  HOST=\$(hostname -I | awk '{print \$1}')
fi
if [[ -z "\$HOST" ]]; then
  echo "failed to resolve local IP for Redis Cluster" >&2
  exit 1
fi
NODES=""
for p in 7000 7001 7002; do
  NODES="\$NODES \$HOST:\$p"
done
echo yes | docker exec -i chronon-redis-7000 redis-cli --cluster create \$NODES --cluster-replicas 0
REMOTE

CLUSTER_URLS="redis://${DATA_IP}:7000,redis://${DATA_IP}:7001,redis://${DATA_IP}:7002"
export CHRONON_REDIS_CLUSTER_URLS="$CLUSTER_URLS"
export CHRONON_REDIS_HASH_TAGS=1

{
  echo "CHRONON_REDIS_CLUSTER_URLS=${CLUSTER_URLS}"
  echo "CHRONON_REDIS_HASH_TAGS=1"
  echo "REDIS_CLUSTER_IP=${DATA_IP}"
} >> "$SF/instances.env"

echo "Redis Cluster ready: ${CLUSTER_URLS}"
