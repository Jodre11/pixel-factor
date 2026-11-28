#!/usr/bin/env bash
# Drive the HTTP service end to end: challenge, enrol, staged enrolment, verify, delete.
#
#   scripts/smoke.sh               start a throwaway service, test it, stop it
#   scripts/smoke.sh <base-url>    test a service that is already running
#
# The token comes from PF_API_TOKEN (default dev-token). Exits non-zero on the
# first step that does not answer as expected.
set -euo pipefail

token="${PF_API_TOKEN:-dev-token}"
base="${1:-}"
work=""
pid=""

cleanup() {
    if [[ -n "$pid" ]]; then kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; fi
    if [[ -n "$work" ]]; then rm -rf "$work"; fi
}
trap cleanup EXIT

if [[ -z "$base" ]]; then
    work="$(mktemp -d)"
    cargo build --quiet --locked --release -p pixel-factor-service
    PF_LISTEN=127.0.0.1:0 PF_DB_PATH="$work/smoke.db" PF_API_TOKEN="$token" PF_IMAGES=demo=0.75 \
        PF_DEV_PEPPER_HEX=6465762d6f6e6c792d7065707065722d6e6f742d666f722d70726f64756374696f6e \
        RUST_LOG=warn target/release/pixel-factor-service >"$work/out" 2>"$work/err" &
    pid=$!
    for _ in $(seq 1 100); do
        if grep -q '^listening on ' "$work/out"; then break; fi
        if ! kill -0 "$pid" 2>/dev/null; then cat "$work/err" >&2; exit 1; fi
        sleep 0.1
    done
    base="$(sed -n 's/^listening on //p' "$work/out")"
    if [[ -z "$base" ]]; then echo "smoke: service did not start" >&2; exit 1; fi
fi

user="smoke-$$-$RANDOM"
taps='[[0.10,0.10],[0.30,0.20],[0.50,0.30],[0.70,0.40],[0.90,0.50]]'
wrong='[[0.10,0.10],[0.30,0.20],[0.50,0.30],[0.70,0.40],[0.90,0.60]]'

# step <name> <expected status> <expected body fragment> <method> <path> [body]
step() {
    local name="$1" want_status="$2" want_body="$3" method="$4" path="$5" body="${6:-}"
    local args=(-sS -o /dev/stdout -w '\n%{http_code}' -X "$method" -H "Authorization: Bearer $token")
    if [[ -n "$body" ]]; then args+=(-H 'Content-Type: application/json' --data "$body"); fi
    local out status
    out="$(curl "${args[@]}" "$base$path")"
    status="${out##*$'\n'}"
    out="${out%$'\n'*}"
    if [[ "$status" != "$want_status" || "$out" != *"$want_body"* ]]; then
        echo "smoke: $name: got $status $out, wanted $want_status containing $want_body" >&2
        exit 1
    fi
    echo "ok  $name"
}

step healthz 200 ok GET /healthz
step version 200 '"version"' GET /version
step challenge 200 '"max_taps":8' POST /v1/challenge "{\"user_id\":\"$user\"}"
step enrol 201 '"warnings"' POST /v1/enrol "{\"user_id\":\"$user\",\"image_id\":\"demo\",\"taps\":$taps}"
step "verify, right taps" 200 '"match":true' POST /v1/verify "{\"user_id\":\"$user\",\"taps\":$taps}"
step "verify, wrong taps" 200 '"match":false' POST /v1/verify "{\"user_id\":\"$user\",\"taps\":$wrong}"
step "stage enrolment" 201 '"warnings"' POST /v1/enrolments "{\"user_id\":\"$user\",\"image_id\":\"demo\",\"taps\":$wrong}"
step "verify, still the old taps" 200 '"match":true' POST /v1/verify "{\"user_id\":\"$user\",\"taps\":$taps}"
step "confirm enrolment" 200 '"confirmed":true' POST /v1/enrolments/confirm "{\"user_id\":\"$user\",\"taps\":$wrong}"
step "verify, the new taps" 200 '"match":true' POST /v1/verify "{\"user_id\":\"$user\",\"taps\":$wrong}"
step delete 204 '' DELETE "/v1/users/$user"
step "verify, deleted" 200 '"match":false' POST /v1/verify "{\"user_id\":\"$user\",\"taps\":$wrong}"
echo "smoke: all steps passed against $base"
