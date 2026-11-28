#!/usr/bin/env bash
# Start the Keycloak demo on a free port, check it end to end, then remove it.
#
#   scripts/demo-smoke.sh <service image>    `make demo-smoke` builds the jar and the image first
#
# Checks: Keycloak loaded all three providers and imported the realm; the service answers every call the plugin
# makes, from inside the compose network; the demo image's aspect matches PF_IMAGES; and a login through Keycloak's
# own pages enrols taps, refuses a wrong sequence and accepts the right one.
set -euo pipefail

image="${1:?usage: scripts/demo-smoke.sh <service image>}"
root="$(cd "$(dirname "$0")/.." && pwd)"
compose_file="$root/keycloak/demo/compose.yaml"
project="pf-demo-smoke-$$"
maven="maven@sha256:99e61abcff91a9b1333463bd8451fb18495d6eba9250ac66a338b518f8278320"

export PF_SERVICE_IMAGE="$image" PF_DEMO_PROJECT="$project" PF_DEMO_PUBLISH="127.0.0.1:"

compose() {
    docker compose -f "$compose_file" "$@"
}

cleanup() {
    status=$?
    if [[ $status -ne 0 ]]; then compose logs --no-color --tail 80 >&2 || true; fi
    compose down -v --remove-orphans >/dev/null 2>&1 || true
}
trap cleanup EXIT

# The plugin compiles against one Keycloak version and the demo runs another image; they must be the same release,
# because the SPIs the plugin uses are internal to Keycloak and may change between releases. Dependabot updates the
# two in separate pull requests, so this check fails the one that moves first.
built="$(sed -n 's:.*<keycloak.version>\(.*\)</keycloak.version>.*:\1:p' "$root/keycloak/pom.xml")"
running="$(sed -n 's|.*quay.io/keycloak/keycloak:\([^@]*\)@.*|\1|p' "$compose_file")"
if [[ -z "$built" || "$built" != "$running" ]]; then
    echo "demo-smoke: pom.xml builds against Keycloak '$built' but compose.yaml runs '$running'; move them together" >&2
    exit 1
fi
echo "ok  plugin and demo use Keycloak $built"

compose up -d --quiet-pull
port="$(compose port keycloak 8080 | sed 's/.*://')"
base="http://127.0.0.1:$port"

for _ in $(seq 1 120); do
    if curl -sf -o /dev/null "$base/realms/demo"; then break; fi
    sleep 1
done
curl -sf -o /dev/null "$base/realms/demo" || { echo "demo-smoke: realm not up after 120 s" >&2; exit 1; }
echo "ok  realm imported"

token="$(curl -sf "$base/realms/master/protocol/openid-connect/token" \
    -d grant_type=password -d client_id=admin-cli -d username=admin -d password=admin |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["access_token"])')"
curl -sf -H "Authorization: Bearer $token" "$base/admin/serverinfo" | python3 -c '
import json, sys
providers = json.load(sys.stdin)["providers"]
want = {"authenticator": "pixel-factor", "required-action": "pixel-factor-enrol", "credential": "pixel-factor"}
for spi, provider in want.items():
    if provider not in providers.get(spi, {}).get("providers", {}):
        sys.exit(f"demo-smoke: Keycloak has no {spi} provider {provider}")
'
echo "ok  providers loaded"

python3 - "$root/keycloak/src/main/resources/theme-resources/resources/img/demo.svg" "$compose_file" <<'EOF'
import re, sys
svg = open(sys.argv[1]).read()
_, _, width, height = (float(v) for v in re.search(r'viewBox="([^"]+)"', svg).group(1).split())
declared = float(re.search(r"PF_IMAGES: demo=([0-9.]+)", open(sys.argv[2]).read()).group(1))
if abs(height / width - declared) > 1e-9:
    sys.exit(f"demo-smoke: demo.svg aspect {height / width} differs from PF_IMAGES {declared}")
print("ok  demo image aspect matches PF_IMAGES")
EOF

# The service is not published, so drive it from a container on the compose network, as Keycloak reaches it.
docker run --rm --network "${project}_default" -v "$root/scripts:/scripts:ro" -e PF_API_TOKEN=dev-token \
    "$maven" bash /scripts/smoke.sh http://service:8080

python3 "$root/keycloak/demo/login_smoke.py" "$base"
