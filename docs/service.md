# HTTP service

`pixel-factor-service` wraps the core in a small HTTP API, owns the stored records and calls the pepper through the core's `PepperProvider`. It is meant to sit on an internal network behind an identity system such as Keycloak, never on the public internet. See [spec.md](spec.md) section 16 for where it fits.

Status: development build. The pepper is an in-memory key; there is no KMS integration and no deployment yet. For the container image, logs, health and alerts, see [OPERATIONS.md](OPERATIONS.md).

## Run it

```
make serve
```

`make serve` starts the service with a development pepper, a demo image, an API token of `dev-token` and a database under `target/`. It prints `listening on http://<address>` on stdout. Override any setting on the command line, for example `make serve PF_LISTEN=127.0.0.1:8080`.

`make smoke` starts a throwaway copy on a free port with its own database, drives challenge, enrol, staged enrolment, verify and delete over HTTP, and stops it. `scripts/smoke.sh <base-url>` runs the same steps against a copy that is already running. No external service or credential is involved.

## Configuration

Everything comes from the environment. Nothing is hard-coded, so several copies can run side by side.

| Variable | Required | Meaning |
|---|---|---|
| `PF_LISTEN` | no | Address to bind. Default `127.0.0.1:0`, which picks a free port and prints it. |
| `PF_DB_PATH` | yes | SQLite database file. Created if missing. |
| `PF_API_TOKEN` | yes | Bearer token every `/v1` request must carry. |
| `PF_IMAGES` | yes | The curated images, as `id=aspect` pairs separated by commas, for example `harbour=0.75`. Aspect is height over width. |
| `PF_DEV_PEPPER_HEX` | yes, for now | Pepper key as hex, at least 32 bytes. Development only: production needs a KMS, which is not built yet. The service logs a warning at start. |
| `PF_MAX_FAILURES` | no | Failed verifications before a user id is locked. Default 5. |
| `PF_LOCKOUT_BASE_SECS` | no | Length of the first lock. Default 60. Each further failure doubles it. |
| `PF_LOCKOUT_MAX_SECS` | no | Longest lock. Default 3600. |
| `PF_FAILURE_WINDOW_SECS` | no | Failures older than this are forgotten. Default 86400. Must be at least `PF_LOCKOUT_MAX_SECS`. |
| `PF_MAX_CONCURRENT` | no | Verifications and enrolments run at once. Each uses 64 MiB for Argon2id. Default: the number of CPUs. |
| `RUST_LOG` | no | Log filter. Default `info`. |
| `PF_LOG_FORMAT` | no | `json` for one JSON object per line; anything else for text. The image sets `json`. |

`PF_BUILD_COMMIT` is read at build time, not run time: `make serve` sets it to the current commit, and `/version` reports it.

## API

All `/v1` endpoints take and return JSON and need `Authorization: Bearer <PF_API_TOKEN>`. Taps are `[x, y]` pairs in units of image width.

| Method and path | Body | Answer |
|---|---|---|
| `POST /v1/challenge` | `{"user_id"}` | `200 {"image_id", "max_taps"}`: the image to show, and the most taps the policy accepts. |
| `POST /v1/enrol` | `{"user_id", "image_id", "taps"}` | `201 {"warnings"}`, or `422 {"error"}` if the sequence is not acceptable. Replaces any existing record and clears a lockout. |
| `POST /v1/enrolments` | `{"user_id", "image_id", "taps"}` | `201 {"warnings"}`, or `422 {"error"}`. Stores a pending enrolment, replacing any earlier one. The live record and any lockout are untouched. |
| `POST /v1/enrolments/confirm` | `{"user_id", "taps"}` | `200 {"confirmed"}`. On a match the pending enrolment replaces the live record and clears a lockout. Either way the pending enrolment is gone. |
| `POST /v1/verify` | `{"user_id", "taps"}` | `200 {"match", "locked"}`. |
| `DELETE /v1/users/{user_id}` | none | `204`. Also removes a pending enrolment. |
| `GET /healthz` | none | `200 ok`. No token needed. |
| `GET /version` | none | `200 {"version", "commit"}`. No token needed. |

## Decisions

- **Unknown users look like known ones.** `/v1/challenge` returns an image for any user id. For an unknown one, the image is chosen by a keyed hash of the id, so it is stable but reveals nothing. `/v1/verify` for an unknown user returns no match.
- **Timing does not reveal whether a user exists.** Records do not hold the tap count (format version 2), so verification costs the same for any record and depends only on the number of taps submitted. For an unknown user the service verifies against a decoy record for the image that user would be shown, and discards the result. Known and unknown ids run the same database statements. Small differences remain, such as a cache-warm row for an existing user; they are far below the cost of Argon2id.
- **Lockout, the same for every id.** After `PF_MAX_FAILURES` failures within `PF_FAILURE_WINDOW_SECS`, the id is locked for `PF_LOCKOUT_BASE_SECS`, doubling with each further failure up to `PF_LOCKOUT_MAX_SECS`. While locked, verification answers `{"match": false, "locked": true}`, even for the right taps. A success, re-enrolment or deletion clears the count. Failures are counted for unknown ids too, so the `locked` flag does not show whether an account exists. A lock that lasts until re-enrolment was rejected: anyone could lock any user out for good. Rows for ids that fail and then go quiet are pruned once their window passes, so made-up ids cannot grow the database without bound. A locked request still runs a full verification, so timing does not show the lock. Each attempt is counted before its taps are checked and refunded only on a match, so a burst of concurrent guesses cannot all get in under the limit.
- **Brute force.** Guessing one user is capped by the lockout. With the defaults, an attacker gets 5 guesses, then about one an hour: roughly 30 a day. Even a five-tap sequence offers far more than that many choices (spec section 9), so online guessing of a single account does not work. Spreading guesses across many users (password spraying) is limited per id the same way, but the service cannot throttle a *source*: its only caller is the identity system, and it never sees the end user's address. Per-source throttling belongs in front of it, in Keycloak's brute-force detection and at the network edge. The service's own limit on concurrent work keeps a flood from exhausting memory, at the cost of slower answers for everyone.
- **Lockout as denial of service.** Because failures are remembered for the whole window, one wrong try after a lock expires renews it, so a stranger who knows a username could keep its owner locked out cheaply. The service cannot tell that stranger from the user. The caller must therefore run the tap step only after a possession factor (a passkey or a one-time code) has passed; the Keycloak plugin will enforce that order.
- **Staged enrolment.** `/v1/enrol` replaces a record at once, so an identity system could not let a user repeat a new sequence before it replaced the old one. `/v1/enrolments` holds the new record aside until `/v1/enrolments/confirm` sees the same taps. The pending enrolment is taken out of the store before the taps are checked, so it gets exactly one attempt, even under concurrent requests: a wrong confirmation leaves nothing to guess against, and needs no lockout of its own. The record is built from the first entry; the confirmation only proves the user can repeat it. Pending enrolments expire after 10 minutes and are pruned. Confirming with nothing pending answers `confirmed: false` without a decoy: the caller has already authenticated the user and knows they are enrolling, so the timing reveals nothing. `/v1/enrol` stays for tests and direct provisioning.
- **`max_taps` is public.** It is the policy's limit, the same for every user, already documented, and lets the tap page send a fixed-length body so a network observer cannot count taps.
- **A shared API token** authenticates the caller. It is compared in constant time. Mutual TLS is the likely production answer.
- **Bounded work.** Bodies over 16 KiB are refused, and a semaphore caps concurrent Argon2id runs, so a burst of requests cannot exhaust memory.
- **Logs never hold taps, records or the pepper.** They hold the user id and the outcome. User ids with control characters are refused, and ids are logged escaped, so an id cannot forge a log line.
- **SQLite through `rusqlite`,** one connection behind a mutex. Enough for one instance; a shared store is a later decision (spec section 19).

## Not yet

- KMS-backed `PepperProvider`, and pepper rotation.
- A published image and a deployment target. The image is built and scanned in CI but not pushed anywhere.
- Metrics and alert definitions. [OPERATIONS.md](OPERATIONS.md) lists the signals to alert on from the logs meanwhile.
- Per-source rate limiting, if the identity system in front does not provide it.
