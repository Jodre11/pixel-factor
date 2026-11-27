#!/usr/bin/env bash
# Run the service image with development settings, wait until its health check
# passes, drive it with scripts/smoke.sh, then remove it.
#
#   scripts/container-smoke.sh <image>    `make container-smoke` passes the image `make image` built
set -euo pipefail

image="${1:?usage: scripts/container-smoke.sh <image>}"
token="dev-token"
name="pf-smoke-$$"

cleanup() {
    docker rm -f "$name" >/dev/null 2>&1 || true
}
trap cleanup EXIT

docker run -d --name "$name" -p 127.0.0.1::8080 \
    -e PF_API_TOKEN="$token" \
    -e PF_IMAGES=demo=0.75 \
    -e PF_DEV_PEPPER_HEX=6465762d6f6e6c792d7065707065722d6e6f742d666f722d70726f64756374696f6e \
    "$image" >/dev/null

# Startup builds a decoy record per image, so allow it a little while.
for _ in $(seq 1 60); do
    status="$(docker inspect -f '{{.State.Health.Status}}' "$name" 2>/dev/null || echo gone)"
    if [[ "$status" == healthy ]]; then break; fi
    if [[ "$status" == gone || "$(docker inspect -f '{{.State.Running}}' "$name")" != true ]]; then
        docker logs "$name" >&2
        exit 1
    fi
    sleep 1
done
if [[ "$status" != healthy ]]; then
    echo "container-smoke: not healthy after 60 s" >&2
    docker logs "$name" >&2
    exit 1
fi

port="$(docker port "$name" 8080/tcp | head -n 1 | sed 's/.*://')"
PF_API_TOKEN="$token" scripts/smoke.sh "http://127.0.0.1:$port"
