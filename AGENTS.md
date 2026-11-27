# Working in this repo

Read this first. It is the entry point for agents and new contributors. `CLAUDE.md` imports it.

## What this is

pixel-factor is a knowledge factor: a user proves who they are by tapping an ordered sequence of points on an image. The stored record is breach resistant. See [docs/PRODUCT.md](docs/PRODUCT.md) for who it serves and why, and [docs/spec.md](docs/spec.md) for the design.

## Before changing behaviour

Check the change against the principles and non-goals in [docs/PRODUCT.md](docs/PRODUCT.md). If a request conflicts with one, say so and stop before writing code. A change that weakens breach resistance is never a refactor.

## Commands

Everything goes through the Makefile. `make help` lists the targets.

| Command | Use it to |
|---|---|
| `make setup` | Prepare a fresh checkout. Installs the git hooks, fetches dependencies and installs the analysis tools. Runs automatically at session start and in the devcontainer. The first run builds `cargo-deny` and takes a few minutes; later runs are instant. |
| `make check` | Prove a change is sound. This is what CI runs. See the gates below. |
| `make check-changed` | Check only what changed. Use this while iterating. The pre-commit hook runs the staged version. |
| `make fmt` | Format the code. |
| `make test` | Run the tests only, including the property tests. |
| `make coverage` | Line coverage with a floor for each crate. CI runs it. Needs `make tools-deep` once. |
| `make mutants` | Mutation testing. Slow; CI runs it weekly. Needs `make tools-deep` once. |
| `make fuzz` | Fuzz `verify`. CI runs it weekly. Needs `make tools-deep` once and a nightly toolchain. |
| `make advisories` | Check dependencies for known vulnerabilities. Needs network. CI runs it weekly. |
| `make kat` | Print the known-answer record. |
| `make timing` | Time one enrolment and verification at production settings. |
| `make serve` | Run the HTTP service locally with development settings. Prints its address. |
| `make smoke` | Start a throwaway service, drive enrol and verify over HTTP, stop it. |
| `make image` | Build the service container image. Needs Docker. `scripts/container-smoke.sh` then runs and drives it. |
| `make iac-scan` | Trivy misconfiguration scan of the `Dockerfile` and workflows. |
| `make image-scan` | Trivy vulnerability scan of the built image. |

The toolchain is pinned in `rust-toolchain.toml`; rustup installs it on first use. `make setup` installs everything else. The one thing it may not manage is `shellcheck` on a machine with neither Homebrew nor apt: it says so, and you install it once by hand.

## Quality gates

`make check` runs all of these and fails on any:

| Gate | What it catches |
|---|---|
| `make fmt-check` | Unformatted code. |
| `make lint` | Clippy warnings, plus the stricter lints set at the top of `core/src/lib.rs`: no `unwrap`, `expect`, `panic`, unchecked indexing or lossy `as` casts in library code, and no `unsafe` at all. |
| `make test` | Unit, integration and property tests. Property tests state the invariants: taps inside tolerance always match, taps outside never do, and `verify` never errors or panics on any tap input. |
| `make doc` | Missing or broken API documentation. Every public item must be documented. |
| `make docs-check` | Markdown that names a `make` target or file that does not exist. |
| `make deny` | Dependencies with a licence outside the allow list in `deny.toml`, from an unknown source, or with wildcard versions. |
| `make shellcheck` | Mistakes in the shell scripts. |
| `make fuzz-check` | A fuzz target that no longer compiles. |

Slower gates run in CI but not in `make check`: `make smoke`, `make coverage` and a container job (Trivy scans of the `Dockerfile` and image, then `scripts/container-smoke.sh`) on every pull request, and `make mutants`, `make fuzz` and `make advisories` weekly.

If a gate blocks a change you believe is right, fix the code or raise it. Do not silence a lint with `#[allow]` unless the reason is written beside it, as in `core/src/geometry.rs`.

## Layout

| Path | What |
|---|---|
| `core/` | Rust library `pixel-factor-core`: geometry, enrolment, verification. No I/O. |
| `core/src/geometry.rs` | Hexagon lattice maths. The only place geometry lives. |
| `core/src/lib.rs` | `enrol`, `verify`, `Record`, `Policy`, `PepperProvider`. |
| `core/tests/core.rs` | Example-based tests. |
| `core/tests/properties.rs` | Property tests. |
| `service/` | HTTP service `pixel-factor-service`: axum and SQLite around the core. See `docs/service.md`. |
| `docs/service.md` | Service configuration, API and decisions. |
| `docs/OPERATIONS.md` | Deploying the service, its version, health, logs and alerts. |
| `Dockerfile` | Service container image: distroless, non-root, base images pinned by digest. |
| `fuzz/` | Fuzz target for `verify`. A workspace member that shares `Cargo.lock`, excluded from the default build and tests. |
| `deny.toml` | Dependency policy. |
| `core/tests/vectors/kat2.json` | Known-answer vector. Locks the record format. |
| `docs/spec.md` | Design decisions and reasoning. |
| `docs/core-format.md` | Byte-level formats. |
| `docs/PRODUCT.md` | Audience, problem, principles, non-goals. |
| `docs/THREAT-MODEL.md` | Attackers, protections, known limits, operator duties. |
| `docs/FAQ.md` | Short answers to common review questions. |
| `scripts/` | Check scripts used by the Makefile, the hook and CI. |

Planned, not yet present: a Keycloak authenticator plugin and a tap demo page.

## Vocabulary

| Term | Meaning |
|---|---|
| Tap | One touch or click on the image. |
| Sequence | The ordered list of taps that forms the secret. |
| Point | An enrolled tap position. |
| Revisit | A tap that returns to an earlier point. Snapped to that exact point at enrolment. |
| r | Guaranteed tolerance: the hexagon inradius, as a fraction of image width. |
| s | Worst-case accepted distance: the hexagon circumradius, 2r / sqrt(3). |
| Cell | One lattice cell. A point's cell is the secret. |
| Index | Which cell a point is in. Secret. Two i32 values (q, t). |
| Offset | Where a point sits inside its cell. Public. Two u16 values. |
| Lattice coordinates | (q, t), a position expressed in the hexagon lattice basis. |
| Salt | 16 random bytes per record. |
| Pepper | A server-side secret held outside the database. The core never holds it in production; a KMS does the HMAC. |
| Verifier | The one stored hash covering the whole sequence. |
| Record | Everything stored for a user. Safe to keep in a database. |
| Context | Image id, r and aspect, bound into every key and the verifier. |
| KAT | Known-answer test. |

Coordinates are in units of image width, never pixels: x runs 0 to 1, y runs 0 to aspect.

## Gotchas

- **Never add an integrity check to stored offsets.** A wrongly decrypted offset must look plausible. Any error or validation there lets an attacker test taps one at a time.
- **Never hash or check points individually.** One verifier covers the whole sequence. No per-tap feedback, anywhere.
- **Never reveal the tap count.** Not in the record (offsets are padded to a fixed length with random fill), not in a response, not in a log, not in timing. Only the documented minimum is public.
- **`verify` returns `Ok(false)` for anything wrong with the taps.** `Err` is only for a malformed record or a pepper failure. Keep it that way.
- **The KAT locks the format.** If `known_answer_vector` fails, you changed the record format. Do not regenerate `kat2.json` to make it pass unless the format change is intended, in which case bump `RECORD_VERSION` too.
- **Domain strings and byte encodings are part of the format.** `pfv1/offset`, `pfv1/kdf`, `pfv1/final`, big-endian integers. See `docs/core-format.md`.
- **Geometry stays in `core/src/geometry.rs`.** A second implementation would round differently at cell edges.
- **Tests use cheap Argon2id settings.** Production defaults are in `Policy::standard`. Do not copy the test settings.
- **British spelling in identifiers:** `enrol`, `Enrolment`. Do not "fix" it.
- **A wrong offset still matches its own tap about half the time.** That is expected, not a bug.

## Running several sessions at once

Safe. The core does no I/O: no ports, no files, no environment variables, no shared state. Tests build everything in memory. Each worktree gets its own `target/` directory.

The service takes its port and database path from the environment. `make serve` binds a free port by default, and `make smoke` uses its own temporary database, so copies in several worktrees do not collide. Do not hard-code either.

## Outside dependencies

None at runtime. `make check` runs entirely offline. The only network use is in `make setup`, once, to fetch dependencies and build the tools, and in `make advisories`, which downloads the advisory database. The KMS is behind the `PepperProvider` trait, and `StaticPepper` is the in-memory substitute for development and tests.

## How work is done here

See [CONTRIBUTING.md](CONTRIBUTING.md). In short:

- Work is tracked in GitHub Issues on this repo.
- Changes go through a pull request to `main`. CI must pass.
- Commits are made under the contributor's own identity. No tool or AI attribution lines in commits or pull requests. CI rejects them.
- Security issues are reported privately: see [SECURITY.md](SECURITY.md).

## Status

Early and unaudited. Nothing here protects real accounts yet.
