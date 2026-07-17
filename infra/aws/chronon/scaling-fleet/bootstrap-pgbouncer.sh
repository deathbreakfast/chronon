#!/usr/bin/env bash
# Install PgBouncer in transaction mode on the Postgres host.
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
  --ip-permissions "IpProtocol=tcp,FromPort=6432,ToPort=6432,UserIdGroupPairs=[{GroupId=${SG_ID}}]" \
  2>/dev/null || true

SSH_KEY="$(bench_ssh_key)"
SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=30 -i "$SSH_KEY")
PG_HOST="ec2-user@${POSTGRES_PUBLIC_IP:-${POSTGRES_IP}}"

ssh "${SSH_OPTS[@]}" "$PG_HOST" bash -s <<'REMOTE'
set -euo pipefail
docker rm -f chronon-pgbouncer 2>/dev/null || true
cat > /tmp/pgbouncer.ini <<'INI'
[databases]
chronon = host=127.0.0.1 port=5432 dbname=chronon user=chronon password=chronon

[pgbouncer]
listen_addr = 0.0.0.0
listen_port = 6432
auth_type = plain
auth_file = /etc/pgbouncer/userlist.txt
pool_mode = session
max_client_conn = 500
default_pool_size = 150
ignore_startup_parameters = extra_float_digits
INI
echo '"chronon" "chronon"' > /tmp/userlist.txt
docker run -d --name chronon-pgbouncer --restart unless-stopped --net host \
  -v /tmp/pgbouncer.ini:/etc/pgbouncer/pgbouncer.ini:ro \
  -v /tmp/userlist.txt:/etc/pgbouncer/userlist.txt:ro \
  edoburu/pgbouncer
for i in $(seq 1 30); do
  python3 - <<'PY' && break
import socket
s=socket.socket(); s.settimeout(1); s.connect(("127.0.0.1",6432)); s.close()
PY
  sleep 1
done
REMOTE

# Route bench traffic through PgBouncer port 6432 on postgres host.
sed -i '/^POSTGRES_IP=/d' "$SF/instances.env" || true
{
  echo "POSTGRES_IP=${POSTGRES_IP}"
  echo "PGBOUNCER_PORT=6432"
  echo "CHRONON_PG_VIA_PGBOUNCER=1"
} >> "$SF/instances.env"

echo "PgBouncer ready on ${POSTGRES_IP}:6432"
