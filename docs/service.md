# HTTP service

`pixel-factor-service` wraps the core in a small HTTP API, owns the stored records and calls the pepper through the core's `PepperProvider`. It is meant to sit on an internal network behind an identity system such as Keycloak, never on the public internet. See [spec.md](spec.md) section 16 for where it fits.

Status: development build. The pepper is an in-memory key; there is no KMS integration and no deployment yet.

## Run it

```
make serve
```

`make serve` starts the service with a development pepper, a demo image, an API token of `dev-token` and a database under `target/`. It prints the address it listens on. `scripts/smoke.sh <base-url>` then enrols a user and verifies them, which shows the whole flow working.

## Configuration

Everything comes from the environment. Nothing is hard-coded, so several copies can run side by side.

| Variable | Required | Meaning |
|---|---|---|
| `PF_LISTEN` | no | Address to bind. Default `127.0.0.1:0`, which picks a free port and prints it. |
| `PF_DB_PATH` | yes | SQLite database file. Created if missing. |
| `PF_API_TOKEN` | yes | Bearer token every `/v1` request must carry. |
| `PF_IMAGES` | yes | The curated images, as `id=aspect` pairs separated by commas, for example `harbour=0.75`. Aspect is height over width. |
| `PF_DEV_PEPPER_HEX` | yes, for now | Pepper key as hex, at least 32 bytes. Development only: production needs a KMS, which is not built yet. The service logs a warning at start. |
| `PF_MAX_FAILURES` | no | Consecutive failed verifications before a user is locked. Default 5. |
| `PF_MAX_CONCURRENT` | no | Verifications and enrolments run at once. Each uses 64 MiB for Argon2id. Default: the number of CPUs. |
| `RUST_LOG` | no | Log filter. Default `info`. |

## API

All `/v1` endpoints take and return JSON and need `Authorization: Bearer <PF_API_TOKEN>`. Taps are `[x, y]` pairs in units of image width.

| Method and path | Body | Answer |
|---|---|---|
| `POST /v1/challenge` | `{"user_id"}` | `200 {"image_id"}`: the image to show. |
| `POST /v1/enrol` | `{"user_id", "image_id", "taps"}` | `201 {"warnings"}`, or `422 {"error"}` if the sequence is not acceptable. Replaces any existing record and clears a lockout. |
| `POST /v1/verify` | `{"user_id", "taps"}` | `200 {"match", "locked"}`. |
| `DELETE /v1/users/{user_id}` | none | `204`. |
| `GET /healthz` | none | `200 ok`. No token needed. |
| `GET /version` | none | `200 {"version", "commit"}`. No token needed. |

## Decisions

- **Unknown users look like known ones.** `/v1/challenge` returns an image for any user id. For an unknown one, the image is chosen by a keyed hash of the id, so it is stable but reveals nothing. `/v1/verify` for an unknown user returns no match.
- **Timing does not reveal whether a user exists.** The core skips Argon2id when the tap count is wrong, so a naive service would answer a known user faster than an unknown one, or the reverse. The service always runs one full verification for any tap count the policy allows: against the user's record when the count matches, and against a decoy record of that count otherwise. Requests with a count outside the policy, or a tap outside the image, are rejected quickly for every user alike.
- **Lockout.** After `PF_MAX_FAILURES` consecutive failures, verification returns `{"match": false, "locked": true}` until the user is enrolled again. The caller is trusted, so it is told; it should show the person a generic message. A locked request still costs a full verification, so timing does not distinguish it.
- **A shared API token** authenticates the caller. It is compared in constant time. Mutual TLS is the likely production answer.
- **Bounded work.** Bodies over 16 KiB are refused, and a semaphore caps concurrent Argon2id runs, so a burst of requests cannot exhaust memory.
- **Logs never hold taps, records or the pepper.** They hold the user id and the outcome.
- **SQLite through `rusqlite`,** one connection behind a mutex. Enough for one instance; a shared store is a later decision (spec section 19).

## Not yet

- KMS-backed `PepperProvider`, and pepper rotation.
- Container image and deployment. Until then there is nothing live, so `/version` on a running copy is the only answer to "what version is this".
- Metrics.
