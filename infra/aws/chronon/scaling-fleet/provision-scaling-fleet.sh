#!/usr/bin/env bash
# Provision scaling fleet EC2 for CH7-D phases.
# d3: launch 4 distinct c6i.large bench hosts (reuses existing data-services).
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHRONON_AWS="$(cd "$SF/.." && pwd)"
PHASE="${1:-d0}"

FLEET_STATE="$CHRONON_AWS/fleet-state.json"
OUT="$SF/instances.env"
IDS_FILE="$SF/instance-ids.txt"
REGION="${AWS_REGION:-us-west-2}"

VPC_ID="$(jq -r '.vpc_id' "$FLEET_STATE")"
SUBNET_ID="$(jq -r '.subnet_id' "$FLEET_STATE")"
SG_ID="$(jq -r '.security_group_id' "$FLEET_STATE")"
DATA_IP="$(jq -r '.data_services.private_ip' "$FLEET_STATE")"
DATA_PUBLIC_IP="$(jq -r '.data_services.public_ip' "$FLEET_STATE")"
DATA_INSTANCE_ID="$(jq -r '.data_services.instance_id' "$FLEET_STATE")"
KEY_NAME="${CHRONON_KEY_NAME:-chronon-bench-key}"
AMI_ID="${CHRONON_AMI_ID:-ami-07ced3ce03c5e117b}"
BENCH_TYPE="${CHRONON_BENCH_INSTANCE_TYPE:-c6i.large}"

SSH_KEY="${CHRONON_SSH_KEY:?set CHRONON_SSH_KEY to the SSH private key path}"

ensure_key_pair() {
  if [[ ! -f "$SSH_KEY" ]]; then
    echo "creating key pair $KEY_NAME -> $SSH_KEY"
    mkdir -p "$(dirname "$SSH_KEY")"
    aws ec2 create-key-pair --region "$REGION" --key-name "$KEY_NAME" \
      --query 'KeyMaterial' --output text > "$SSH_KEY"
    chmod 600 "$SSH_KEY"
  elif ! aws ec2 describe-key-pairs --region "$REGION" --key-names "$KEY_NAME" &>/dev/null; then
    echo "SSH key file exists but AWS key pair $KEY_NAME missing; importing public half"
    aws ec2 import-key-pair --region "$REGION" --key-name "$KEY_NAME" \
      --public-key-material "fileb://${SSH_KEY}.pub" 2>/dev/null || {
      ssh-keygen -y -f "$SSH_KEY" > "${SSH_KEY}.pub"
      aws ec2 import-key-pair --region "$REGION" --key-name "$KEY_NAME" \
        --public-key-material "fileb://${SSH_KEY}.pub"
    }
  fi
}

user_data_bench() {
  local ud="$SF/../user-data/bench.sh"
  base64 -w0 "$ud"
}

launch_bench_hosts() {
  local count="$1"
  local ud
  ud="$(user_data_bench)"
  echo "launching ${count}× ${BENCH_TYPE} bench hosts..." >&2
  aws ec2 run-instances --region "$REGION" \
    --image-id "$AMI_ID" \
    --instance-type "$BENCH_TYPE" \
    --key-name "$KEY_NAME" \
    --subnet-id "$SUBNET_ID" \
    --security-group-ids "$SG_ID" \
    --associate-public-ip-address \
    --count "$count" \
    --user-data "$ud" \
    --tag-specifications \
      "ResourceType=instance,Tags=[{Key=Project,Value=chronon},{Key=Component,Value=scaling-bench},{Key=Phase,Value=${PHASE}},{Key=Name,Value=chronon-bench-d3}]" \
    --query 'Instances[].InstanceId' --output text | tr '\t' '\n'
}

wait_running() {
  local -a ids=("$@")
  if [[ "${#ids[@]}" -eq 0 ]]; then
    echo "wait_running: no instance IDs provided" >&2
    return 1
  fi
  echo "waiting for instances: ${ids[*]}"
  aws ec2 wait instance-running --region "$REGION" --instance-ids "${ids[@]}"
}

write_instances_env_benches() {
  local -a ids=("$@")
  local count="${#ids[@]}"
  local -a priv=()
  local -a pub=()
  local i

  for id in "${ids[@]}"; do
    local info
    info="$(aws ec2 describe-instances --region "$REGION" --instance-ids "$id" \
      --query 'Reservations[0].Instances[0].[PrivateIpAddress,PublicIpAddress]' --output text)"
    priv+=("$(echo "$info" | awk '{print $1}')")
    pub+=("$(echo "$info" | awk '{print $2}')")
  done

  {
    echo "REGION=${REGION}"
    echo "BENCH_COUNT=${count}"
    echo "DATA_IP=${DATA_IP}"
    echo "DATA_PUBLIC_IP=${DATA_PUBLIC_IP}"
    echo "POSTGRES_IP=${DATA_IP}"
    echo "REDIS_IP=${DATA_IP}"
    echo "STORAGE_TOPOLOGY=postgres-redis-colocated"
    echo "DATA_HOST=ec2-user@${DATA_PUBLIC_IP}"
    for i in "${!priv[@]}"; do
      echo "BENCH_${i}_IP=${priv[$i]}"
      echo "BENCH_${i}_PUBLIC_IP=${pub[$i]}"
    done
    echo "CHRONON_BENCH_INSTANCE_TYPE=${BENCH_TYPE}"
    echo "CHRONON_DATA_INSTANCE_TYPE=t3.medium"
    echo "CHRONON_SSH_KEY=${SSH_KEY}"
    echo "PHASE=${PHASE}"
  } > "$OUT"

  {
    echo "# chronon instance manifest $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "$DATA_INSTANCE_ID  # data-services (pre-existing)"
    for id in "${ids[@]}"; do
      echo "$id"
    done
  } > "$IDS_FILE"

  echo "wrote $OUT and $IDS_FILE (${count} bench hosts)"
  echo "bench public IPs: ${pub[*]}"
}

collect_running_benches() {
  aws ec2 describe-instances --region "$REGION" \
    --filters "Name=tag:Name,Values=chronon-bench-d3" \
              "Name=instance-state-name,Values=running" \
    --query 'Reservations[].Instances[].InstanceId' --output text | tr '\t' '\n' | grep '^i-' || true
}

provision_bench_count() {
  local count="$1"
  ensure_key_pair
  DATA_IP="$(jq -r '.data_services.private_ip' "$FLEET_STATE")"
  DATA_PUBLIC_IP="$(jq -r '.data_services.public_ip' "$FLEET_STATE")"
  DATA_INSTANCE_ID="$(jq -r '.data_services.instance_id' "$FLEET_STATE")"

  mapfile -t EXISTING < <(collect_running_benches)
  if [[ "${#EXISTING[@]}" -ge "$count" ]]; then
    echo "reuse ${#EXISTING[@]} existing bench hosts (need ${count})"
    write_instances_env_benches "${EXISTING[@]:0:$count}"
    return 0
  fi

  local need=$((count - ${#EXISTING[@]}))
  echo "have ${#EXISTING[@]} benches; launching ${need} more to reach ${count}"
  mapfile -t NEW_IDS < <(launch_bench_hosts "$need")
  wait_running "${NEW_IDS[@]}"
  local -a ALL=("${EXISTING[@]}" "${NEW_IDS[@]}")
  write_instances_env_benches "${ALL[@]:0:$count}"
  echo "waiting 120s for rustup user-data on new bench hosts..."
  sleep 120
}

case "$PHASE" in
  d3)
    provision_bench_count 4
    ;;
  d5|bench-*)
    COUNT="${CHRONON_BENCH_COUNT:-16}"
    if [[ "$PHASE" =~ ^bench-([0-9]+)$ ]]; then
      COUNT="${BASH_REMATCH[1]}"
    fi
    provision_bench_count "$COUNT"
    ;;
  d0|d1|d2|d4)
    BENCH_IP="$(jq -r '.e2e_runner.private_ip // empty' "$FLEET_STATE" 2>/dev/null || true)"
    if [[ -z "$BENCH_IP" ]]; then
      echo "fleet-state.json missing e2e_runner.private_ip; provision e2e-runner first" >&2
      exit 1
    fi
    BENCH_COUNT=1
    cat > "$OUT" <<EOF
REGION=${REGION}
BENCH_COUNT=${BENCH_COUNT}
DATA_IP=${DATA_IP}
DATA_PUBLIC_IP=${DATA_PUBLIC_IP}
POSTGRES_IP=${DATA_IP}
REDIS_IP=${DATA_IP}
STORAGE_TOPOLOGY=postgres-redis-colocated
DATA_HOST=ec2-user@${DATA_PUBLIC_IP}
BENCH_0_IP=${BENCH_IP}
BENCH_1_IP=${BENCH_IP}
BENCH_2_IP=${BENCH_IP}
BENCH_3_IP=${BENCH_IP}
CHRONON_BENCH_INSTANCE_TYPE=c6i.large
CHRONON_DATA_INSTANCE_TYPE=t3.medium
CHRONON_SSH_KEY=${CHRONON_SSH_KEY:?set CHRONON_SSH_KEY before provision}
PHASE=${PHASE}
EOF
    echo "wrote $OUT for phase $PHASE (legacy single-host mode)"
    ;;
  *)
    echo "unknown phase: $PHASE" >&2
    exit 1
    ;;
esac
