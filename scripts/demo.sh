#!/usr/bin/env bash
# Start the Keycloak demo and print how to log in. `make demo` and `make demo-tunnel` build the jar and the image
# first.
#
#   scripts/demo.sh <service image>             local only, at http://127.0.0.1:8080
#   scripts/demo.sh <service image> --tunnel    also at a public https://*.trycloudflare.com address
#
#   scripts/demo.sh <service image> --stop      stop either, and delete its data
set -euo pipefail

image="${1:?usage: scripts/demo.sh <service image> [--tunnel | --stop]}"
mode="${2:-}"
root="$(cd "$(dirname "$0")/.." && pwd)"
export PF_SERVICE_IMAGE="$image"

if [[ "$mode" == "--stop" ]]; then
    # Name both files so a tunnel container is removed too; the password is not needed to stop.
    PF_DEMO_ADMIN_PASSWORD=unused docker compose -f "$root/keycloak/demo/compose.yaml" \
        -f "$root/keycloak/demo/compose.tunnel.yaml" down -v --remove-orphans
    exit 0
fi

files=(-f "$root/keycloak/demo/compose.yaml")
admin_password="admin"
if [[ "$mode" == "--tunnel" ]]; then
    files+=(-f "$root/keycloak/demo/compose.tunnel.yaml")
    admin_password="$(openssl rand -hex 16)"
    export PF_DEMO_ADMIN_PASSWORD="$admin_password"
elif [[ -n "$mode" ]]; then
    echo "demo: unknown option '$mode'" >&2
    exit 2
fi

compose() {
    docker compose "${files[@]}" "$@"
}

compose up -d
port="$(compose port keycloak 8080 | sed 's/.*://')"
base="http://127.0.0.1:$port"

printf 'Waiting for Keycloak'
for _ in $(seq 1 120); do
    if curl -sf -o /dev/null "$base/realms/demo"; then break; fi
    printf '.'
    sleep 1
done
echo

if [[ "$mode" == "--tunnel" ]]; then
    printf 'Waiting for the tunnel'
    public=""
    reached=""
    for _ in $(seq 1 60); do
        # The quick tunnel's own address; cloudflared also logs api.trycloudflare.com, which it calls to get one.
        public="$(compose logs tunnel 2>/dev/null |
            grep -o 'https://[a-z0-9]*-[a-z0-9-]*\.trycloudflare\.com' | head -n 1 || true)"
        if [[ -n "$public" ]] && curl -sf -o /dev/null "$public/realms/demo"; then
            reached=1
            break
        fi
        printf '.'
        sleep 2
    done
    echo
    if [[ -z "$reached" ]]; then
        echo "demo: the tunnel did not come up. Its log:" >&2
        compose logs --no-color --tail 20 tunnel >&2 || true
        echo "The local demo is still running at $base; stop it with: make demo-stop" >&2
        exit 1
    fi
    base="$public"
fi

cat <<EOF
Log in at:      $base/realms/demo/account/
Username:       demo
One-time code:  add this secret to an authenticator app (TOTP, SHA-1, 6 digits, 30 s):
                OB2WE3DJMMWWIZLNN4WW65DQFVXG65BNMEWXGZLDOJSXI
The first login asks you to choose a tap sequence, then to repeat it.
Admin console:  $base/admin/  (admin / $admin_password)
Stop it with:   make demo-stop
EOF
if [[ "$mode" == "--tunnel" ]]; then
    echo "Public:         anyone with the address can reach this demo. Every other secret in it is public."
fi
