#!/usr/bin/env bash
# Launch data-services if the fleet-state instance is missing/terminated.
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FLEET_STATE="$SF/fleet-state.json"
REGION="${AWS_REGION:-us-west-2}"
KEY_NAME="${CHRONON_KEY_NAME:-chronon-bench-key}"
SSH_KEY="${CHRONON_SSH_KEY:?set CHRONON_SSH_KEY to the SSH private key path}"
AMI_ID="${CHRONON_AMI_ID:-ami-07ced3ce03c5e117b}"

VPC_ID="$(jq -r '.vpc_id' "$FLEET_STATE")"
SUBNET_ID="$(jq -r '.subnet_id' "$FLEET_STATE")"
SG_ID="$(jq -r '.security_group_id' "$FLEET_STATE")"
OLD_ID="$(jq -r '.data_services.instance_id' "$FLEET_STATE")"

state=""
if [[ -n "$OLD_ID" && "$OLD_ID" != "null" ]]; then
  state="$(aws ec2 describe-instances --region "$REGION" --instance-ids "$OLD_ID" \
    --query 'Reservations[0].Instances[0].State.Name' --output text 2>/dev/null || echo terminated)"
fi

if [[ "$state" == "running" || "$state" == "pending" ]]; then
  echo "data-services ${OLD_ID} is ${state}"
  exit 0
fi

echo "launching data-services (prior state: ${state:-missing})..."
UD="$(base64 -w0 "$SF/user-data/data.sh")"
NEW_ID="$(aws ec2 run-instances --region "$REGION" \
  --image-id "$AMI_ID" \
  --instance-type "${CHRONON_DATA_INSTANCE_TYPE:-t3.medium}" \
  --key-name "$KEY_NAME" \
  --subnet-id "$SUBNET_ID" \
  --security-group-ids "$SG_ID" \
  --associate-public-ip-address \
  --user-data "$UD" \
  --tag-specifications \
    "ResourceType=instance,Tags=[{Key=Project,Value=chronon},{Key=Component,Value=data-services},{Key=Name,Value=chronon-data-services}]" \
  --query 'Instances[0].InstanceId' --output text)"

aws ec2 wait instance-running --region "$REGION" --instance-ids "$NEW_ID"
read -r PRIV PUB <<< "$(aws ec2 describe-instances --region "$REGION" --instance-ids "$NEW_ID" \
  --query 'Reservations[0].Instances[0].[PrivateIpAddress,PublicIpAddress]' --output text)"

tmp="$(mktemp)"
jq --arg id "$NEW_ID" --arg priv "$PRIV" --arg pub "$PUB" \
  '.data_services.instance_id = $id | .data_services.private_ip = $priv | .data_services.public_ip = $pub' \
  "$FLEET_STATE" > "$tmp"
mv "$tmp" "$FLEET_STATE"

echo "data-services ready: id=${NEW_ID} priv=${PRIV} pub=${PUB}"
echo "waiting 90s for docker user-data..."
sleep 90
