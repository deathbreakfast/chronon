#!/usr/bin/env bash
# Terminate all chronon-tagged EC2 instances; poll until zero running/pending.
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REGION="${AWS_REGION:-us-west-2}"
TIMEOUT_SEC="${CHRONON_TEARDOWN_TIMEOUT_SEC:-600}"

# Only live/stopped tagged instances — stale instance-ids.txt entries are ignored.
mapfile -t ids < <(aws ec2 describe-instances --region "$REGION" \
  --filters "Name=tag:Project,Values=chronon" \
            "Name=instance-state-name,Values=running,pending,stopping,stopped" \
  --query 'Reservations[].Instances[].InstanceId' --output text | tr '\t' '\n' | grep '^i-' | sort -u)

if [[ "${#ids[@]}" -eq 0 ]]; then
  echo "no chronon instances to terminate"
  exit 0
fi

echo "terminating chronon instances:"
printf '%s\n' "${ids[@]}"
aws ec2 terminate-instances --region "$REGION" --instance-ids "${ids[@]}"

deadline=$(( $(date +%s) + TIMEOUT_SEC ))
while [[ $(date +%s) -lt $deadline ]]; do
  remaining="$(aws ec2 describe-instances --region "$REGION" \
    --filters "Name=tag:Project,Values=chronon" \
              "Name=instance-state-name,Values=running,pending,stopping" \
    --query 'length(Reservations[].Instances[])' --output text)"
  if [[ "$remaining" == "0" || -z "$remaining" ]]; then
    echo "teardown complete: zero chronon instances running/pending"
    exit 0
  fi
  echo "waiting... $remaining instance(s) still active"
  sleep 15
done

echo "teardown timeout: instances may still be terminating" >&2
aws ec2 describe-instances --region "$REGION" \
  --filters "Name=tag:Project,Values=chronon" \
            "Name=instance-state-name,Values=running,pending,stopping" \
  --query 'Reservations[].Instances[].{Id:InstanceId,State:State.Name,Name:Tags[?Key==`Name`].Value|[0]}' \
  --output table >&2
exit 1
