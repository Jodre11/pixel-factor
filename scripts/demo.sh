#!/usr/bin/env bash
# Start the Keycloak demo and print how to log in. `make demo` builds the jar and the image first.
#
#   scripts/demo.sh <service image>
#
# Stop it with: docker compose -f keycloak/demo/compose.yaml down -v
set -euo pipefail

image="${1:?usage: scripts/demo.sh <service image>}"
root="$(cd "$(dirname "$0")/.." && pwd)"
export PF_SERVICE_IMAGE="$image"

docker compose -f "$root/keycloak/demo/compose.yaml" up -d
port="$(docker compose -f "$root/keycloak/demo/compose.yaml" port keycloak 8080 | sed 's/.*://')"
base="http://127.0.0.1:$port"

printf 'Waiting for Keycloak'
for _ in $(seq 1 120); do
    if curl -sf -o /dev/null "$base/realms/demo"; then break; fi
    printf '.'
    sleep 1
done
echo

cat <<EOF
Log in at:      $base/realms/demo/account/
Username:       demo
One-time code:  add this secret to an authenticator app (TOTP, SHA-1, 6 digits, 30 s):
                OB2WE3DJMMWWIZLNN4WW65DQFVXG65BNMEWXGZLDOJSXI
The first login asks you to choose a tap sequence, then to repeat it.
Admin console:  $base/admin/  (admin / admin)
Stop it with:   docker compose -f keycloak/demo/compose.yaml down -v
EOF
