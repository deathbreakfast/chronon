#!/usr/bin/env bash
# Launch dedicated Redis and Postgres EC2 hosts (real split topology).
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHRONON_AWS="$(cd "$SF/.." && pwd)"
FLEET_STATE="$CHRONON_AWS/fleet-state.json"
REGION="${AWS_REGION:-us-west-2}"
KEY_NAME="${CHRONON_KEY_NAME:-chronon-bench-key}"
AMI_ID="${CHRONON_AMI_ID:-ami-07ced3ce03c5e117b}"
REDIS_TYPE="${CHRONON_REDIS_INSTANCE_TYPE:-c6i.large}"
PG_TYPE="${CHRONON_POSTGRES_INSTANCE_TYPE:-c6i.large}"

VPC_ID="$(jq -r '.vpc_id' "$FLEET_STATE")"
SUBNET_ID="$(jq -r '.subnet_id' "$FLEET_STATE")"
SG_ID="$(jq -r '.security_group_id' "$FLEET_STATE")"

launch_one() {
  local name="$1"
  local itype="$2"
  aws ec2 run-instances --region "$REGION" \
    --image-id "$AMI_ID" \
    --instance-type "$itype" \
    --key-name "$KEY_NAME" \
    --subnet-id "$SUBNET_ID" \
    --security-group-ids "$SG_ID" \
    --associate-public-ip-address \
    --tag-specifications \
      "ResourceType=instance,Tags=[{Key=Project,Value=chronon},{Key=Component,Value=${name}},{Key=Name,Value=${name}}]" \
    --query 'Instances[0].InstanceId' --output text
}

echo "launching split data tier: redis=${REDIS_TYPE} postgres=${PG_TYPE}..."

existing_redis="$(aws ec2 describe-instances --region "$REGION" \
  --filters "Name=tag:Name,Values=chronon-redis" "Name=instance-state-name,Values=running,pending" \
  --query 'Reservations[0].Instances[0].InstanceId' --output text 2>/dev/null || true)"
existing_pg="$(aws ec2 describe-instances --region "$REGION" \
  --filters "Name=tag:Name,Values=chronon-postgres" "Name=instance-state-name,Values=running,pending" \
  --query 'Reservations[0].Instances[0].InstanceId' --output text 2>/dev/null || true)"

if [[ -n "$existing_redis" && "$existing_redis" != "None" && -n "$existing_pg" && "$existing_pg" != "None" ]]; then
  echo "reuse split hosts redis=${existing_redis} postgres=${existing_pg}"
  REDIS_ID="$existing_redis"
  PG_ID="$existing_pg"
else
  REDIS_ID="$(launch_one chronon-redis "$REDIS_TYPE")"
  PG_ID="$(launch_one chronon-postgres "$PG_TYPE")"
  aws ec2 wait instance-running --region "$REGION" --instance-ids "$REDIS_ID" "$PG_ID"
fi

read -r R_PRIV R_PUB <<< "$(aws ec2 describe-instances --region "$REGION" --instance-ids "$REDIS_ID" \
  --query 'Reservations[0].Instances[0].[PrivateIpAddress,PublicIpAddress]' --output text)"
read -r P_PRIV P_PUB <<< "$(aws ec2 describe-instances --region "$REGION" --instance-ids "$PG_ID" \
  --query 'Reservations[0].Instances[0].[PrivateIpAddress,PublicIpAddress]' --output text)"

echo "redis: ${REDIS_ID} ${R_PRIV}/${R_PUB}"
echo "postgres: ${PG_ID} ${P_PRIV}/${P_PUB}"

# shellcheck disable=SC1091
source "$SF/lib/tier-common.sh"
# shellcheck disable=SC1091
source "$SF/instances.env" 2>/dev/null || true
export BENCH_COUNT="${BENCH_COUNT:-4}"
tier_write_instances_env_split "$R_PRIV" "$R_PUB" "$P_PRIV" "$P_PUB" "split-${REDIS_TYPE}"

echo "waiting 90s for SSH..."
if [[ "${CHRONON_SKIP_SPLIT_BOOTSTRAP:-}" != "1" ]]; then
  sleep 90
  "$SF/bootstrap-data-split.sh"
else
  echo "skip split bootstrap (CHRONON_SKIP_SPLIT_BOOTSTRAP=1)"
fi

echo "split data tier ready"
