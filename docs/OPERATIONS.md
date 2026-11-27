# Operating the service

How to deploy `pixel-factor-service`, tell what is running, read its logs and know when it needs attention. For the API and configuration, see [service.md](service.md).

Status: nothing is deployed. There is no production pepper yet (a KMS-backed `PepperProvider` is still to be built), so the service must not protect real accounts. This document describes how a deployment works once that exists, and how to run the image today for testing.

## Deploy

The unit of deployment is the container image built by `make image` from the `Dockerfile`:

- A distroless base with no shell or package manager, running as an unprivileged user (uid 65532).
- Listens on port 8080. Keeps its SQLite database in the `/data` volume, which must be persistent and private to one instance.
- One instance per database. SQLite is opened by a single process; do not point two instances at one volume.
- Configuration is entirely environment variables (see [service.md](service.md#configuration)). Supply `PF_API_TOKEN` and the pepper from a secret store, never in the image or a committed file.
- Stops cleanly on SIGTERM: it finishes requests in flight and exits.
- Reachable only from the identity system that calls it, on an internal network. It is not meant to face the internet.

To run the image locally with development settings and drive it end to end:

```
make image
make container-smoke
```

CI builds the image, scans it and the `Dockerfile` with Trivy, and runs that smoke test on every pull request. The image is not yet published to a registry. Publishing, and the deployment target, are decisions still to make.

## What version is live

- `GET /version` answers `{"version", "commit"}`: the crate version and the git commit the binary was built from. It needs no token.
- The image carries the same commit in the `org.opencontainers.image.revision` label: `docker inspect -f '{{index .Config.Labels "org.opencontainers.image.revision"}}' <image>`.
- The `started` log line repeats both.

## Health

- `GET /healthz` answers `200 ok` once the service is listening. It needs no token.
- The image's `HEALTHCHECK` runs `pixel-factor-service healthcheck`, which calls `/healthz` on the local port. Orchestrators that ignore Docker health checks should probe `/healthz` directly.
- Startup takes a moment: the service builds one decoy record per curated image, each a full Argon2id run.

## Logs

In the image, logs are JSON, one object per line, on stderr (`PF_LOG_FORMAT=json`). Outside it they are plain text. `RUST_LOG` sets the level (default `info`).

| Event (`fields.message`) | Level | Fields | Meaning |
|---|---|---|---|
| `started` | info | `version`, `commit`, `images` | The service is up. |
| `using the development pepper ...` | warn | | Always logged today. In production its presence is an incident. |
| `enrolled` | info | `user_id`, `image_id` | A record was written. |
| `verified` | info | `user_id`, `outcome` (`match`, `no_match`, `locked`), `failures`, `locked` | One verification. |
| `deleted` | info | `user_id` | A user was removed. |
| `internal error` | error | `error` | A request failed with a 500. The caller saw only "internal error". |
| `stopped` | info | | Clean shutdown. |
| `failed to start` | error | `error` | Bad configuration or an unusable database. The process exits non-zero. |

Logs never contain taps, records, the pepper or the API token. User ids are logged, escaped. Treat the logs as personal data.

## Dashboards and alerts

The service has no metrics endpoint yet. Until it does, derive these from the logs:

| Signal | Source | Alert when |
|---|---|---|
| Up | `/healthz`, or the container health status | Unhealthy for more than a minute. |
| Errors | `internal error` events | Any sustained rate. Each one is a bug or a broken dependency. |
| Locks | `verified` with `outcome=locked` | A sharp rise: someone is guessing, at one account or many. |
| Failures across many users | `verified` with `outcome=no_match`, grouped by `user_id` | Many distinct ids failing at once: password spraying. Throttle at the source (see [service.md](service.md#decisions)). |
| Latency | the caller's request timings | Rising steadily: Argon2id work is queueing behind `PF_MAX_CONCURRENT`. Add CPUs or instances with their own databases. |
| Development pepper | the warning at start | Any occurrence in production. |

Metrics, an operations dashboard and alert definitions belong with the deployment and are not built yet.

## Routine tasks

- **Back up the database** with SQLite's online backup, for example `sqlite3 /data/pixel-factor.db ".backup '/backup/pf.db'"`, from a sidecar that mounts the volume. Copying the file alone can miss the write-ahead log. The records are safe to store without the pepper, but the backup is still personal data.
- **Unlock a user** by waiting for the lock to expire (at most `PF_LOCKOUT_MAX_SECS`), or by enrolling them again.
- **Rotate the API token**: set the new `PF_API_TOKEN` and restart. The caller must switch at the same moment; there is no overlap period yet.
- **Change the curated images**: add an image to `PF_IMAGES` and restart. Do not remove or change the aspect of an image that has enrolled users: their records name it, and verification would fail.
