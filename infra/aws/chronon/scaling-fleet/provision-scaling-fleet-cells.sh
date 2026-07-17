#!/usr/bin/env bash
# Provision N independent postgres-redis data cells for T5 multi-cell fleet.
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHRONON_AWS="$(cd "$SF/.." && pwd)"
FLEET_STATE="$CHRONON_AWS/fleet-state.json"
REGION="${AWS_REGION:-us-west-2}"
N="${1:-4}"
KEY_NAME="${CHRONON_KEY_NAME:-chronon-bench-key}"
AMI_ID="${CHRONON_AMI_ID:-ami-07ced3ce03c5e117b}"
CELL_TYPE="${CHRONON_CELL_INSTANCE_TYPE:-t3.medium}"

VPC_ID="$(jq -r '.vpc_id' "$FLEET_STATE")"
SUBNET_ID="$(jq -r '.subnet_id' "$FLEET_STATE")"
SG_ID="$(jq -r '.security_group_id' "$FLEET_STATE")"

OUT="$SF/instances.cells.env"
echo "REGION=${REGION}" > "$OUT"
echo "CELL_COUNT=${N}" >> "$OUT"
echo "DATA_TIER_PROFILE=multicell-t5-${CELL_TYPE}" >> "$OUT"

# Install docker via cloud-init so SSH bootstrap can start containers.
user_data_cell() {
  base64 -w0 <<'UD'
#!/bin/bash
set -euo pipefail
dnf install -y docker
systemctl enable --now docker
usermod -aG docker ec2-user
UD
}

find_running_cell() {
  local idx="$1"
  aws ec2 describe-instances --region "$REGION" \
    --filters "Name=tag:Name,Values=chronon-cell-${idx}" \
              "Name=instance-state-name,Values=running" \
    --query 'Reservations[0].Instances[0].InstanceId' --output text 2>/dev/null || true
}

launch_cell() {
  local idx="$1"
  local id priv pub
  id="$(find_running_cell "$idx")"
  if [[ -n "$id" && "$id" != "None" ]]; then
    echo "reuse cell ${idx}: ${id}"
  else
    id="$(aws ec2 run-instances --region "$REGION" \
      --image-id "$AMI_ID" \
      --instance-type "$CELL_TYPE" \
      --key-name "$KEY_NAME" \
      --subnet-id "$SUBNET_ID" \
      --security-group-ids "$SG_ID" \
      --associate-public-ip-address \
      --user-data "$(user_data_cell)" \
      --tag-specifications \
        "ResourceType=instance,Tags=[{Key=Project,Value=chronon},{Key=Component,Value=cell-${idx}},{Key=Name,Value=chronon-cell-${idx}}]" \
      --query 'Instances[0].InstanceId' --output text)"
    echo "launched cell ${idx}: ${id}"
    aws ec2 wait instance-running --region "$REGION" --instance-ids "$id"
  fi
  read -r priv pub <<< "$(aws ec2 describe-instances --region "$REGION" --instance-ids "$id" \
    --query 'Reservations[0].Instances[0].[PrivateIpAddress,PublicIpAddress]' --output text)"
  echo "CELL_${idx}_INSTANCE_ID=${id}" >> "$OUT"
  echo "CELL_${idx}_POSTGRES_IP=${priv}" >> "$OUT"
  echo "CELL_${idx}_POSTGRES_PUBLIC_IP=${pub}" >> "$OUT"
  echo "CELL_${idx}_REDIS_IP=${priv}" >> "$OUT"
  echo "CELL_${idx}_REDIS_PUBLIC_IP=${pub}" >> "$OUT"
  echo "CELL_${idx}_KEY_PREFIX=cell${idx}" >> "$OUT"
  echo "cell ${idx}: ${id} ${priv}/${pub}"
}

for i in $(seq 0 $((N - 1))); do
  launch_cell "$i"
done

echo "waiting for SSH + docker on cells..."
# shellcheck disable=SC1091
source "$SF/lib/bench-fleet.sh"
SSH_KEY="$(bench_ssh_key)"
SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=30 -i "$SSH_KEY")

# shellcheck disable=SC1091
source "$OUT"
for i in $(seq 0 $((N - 1))); do
  pub_var="CELL_${i}_POSTGRES_PUBLIC_IP"
  host="ec2-user@${!pub_var}"
  echo "bootstrapping docker services on cell ${i} (${!pub_var})..."
  ok=0
  for attempt in $(seq 1 60); do
    if ssh "${SSH_OPTS[@]}" "$host" bash -s <<'REMOTE'
set -euo pipefail
# Ensure docker is installed (user-data may still be running).
if ! command -v docker >/dev/null 2>&1; then
  sudo dnf install -y docker
  sudo systemctl enable --now docker
  sudo usermod -aG docker ec2-user || true
fi
sudo systemctl start docker
sudo docker rm -f chronon-redis chronon-postgres 2>/dev/null || true
sudo docker run -d --name chronon-redis --restart unless-stopped -p 6379:6379 redis:7-alpine
sudo docker run -d --name chronon-postgres --restart unless-stopped \
  -e POSTGRES_USER=chronon -e POSTGRES_PASSWORD=chronon -e POSTGRES_DB=chronon \
  -p 5432:5432 postgres:16-alpine
for j in $(seq 1 30); do
  sudo docker exec chronon-redis redis-cli ping 2>/dev/null | grep -q PONG && \
    sudo docker exec chronon-postgres pg_isready -U chronon -d chronon >/dev/null 2>&1 && exit 0
  sleep 2
done
exit 1
REMOTE
    then
      ok=1
      break
    fi
    sleep 10
  done
  if [[ "$ok" -ne 1 ]]; then
    echo "failed to bootstrap cell ${i} (${!pub_var})" >&2
    exit 1
  fi
done

echo "provisioned ${N} cells -> ${OUT}"
