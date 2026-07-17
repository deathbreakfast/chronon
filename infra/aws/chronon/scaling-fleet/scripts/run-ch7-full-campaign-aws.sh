#!/usr/bin/env bash
# Full CH7 hyperscale campaign D0→D4.
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPTS="$SF/scripts"

"$SF/provision-scaling-fleet.sh" d0
"$SF/bootstrap-data.sh"
"$SCRIPTS/run-ch7-d0-aws.sh"

"$SF/provision-scaling-fleet.sh" d1
"$SCRIPTS/run-ch7-d1-aws.sh"

"$SF/provision-scaling-fleet.sh" d2
"$SCRIPTS/run-ch7-d2-aws.sh"

"$SF/provision-scaling-fleet.sh" d3
"$SCRIPTS/run-ch7-d3-aws.sh"

"$SF/provision-scaling-fleet.sh" d4
"$SCRIPTS/run-ch7-d4-aws.sh"

echo "CH7 hyperscale campaign complete"
