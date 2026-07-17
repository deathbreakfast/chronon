# Teardown

```bash
# Preferred: scaling-fleet helper (reads instance tags / generated instance-ids)
./infra/aws/chronon/scaling-fleet/teardown-chronon-fleet.sh

# Or terminate by Project tag
aws ec2 describe-instances --filters "Name=tag:Project,Values=chronon" \
  "Name=instance-state-name,Values=running,pending,stopping,stopped" \
  --query 'Reservations[].Instances[].InstanceId' --output text \
  | xargs -r aws ec2 terminate-instances --instance-ids

# Optional: delete VPC resources after instances terminate
# Keep committed reports under profiling/chronon-bench/reports/
```
