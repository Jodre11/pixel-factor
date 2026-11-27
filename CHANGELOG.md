# Changelog

Notable changes, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Nothing has been released yet; versions will follow [Semantic Versioning](https://semver.org/) once something is.

A change to the stored record format is always called out here, with the new `RECORD_VERSION`.

## Unreleased

### Changed

- **Longer sequences, free revisits.** `Policy::max_revisits` is replaced by `max_taps`. The standard policy allows 5 to 8 taps with at least 5 distinct points, and any number of revisits that are not adjacent. The record format is unchanged.
- **Record format version 2.** `enc_offsets` is always 124 bytes, padded with random fill, so a stored record no longer reveals how many taps it has. Verification tries any submitted count from 1 to 32 at the same cost. Version 1 records are rejected. `enrol_with_salt` is now `enrol_deterministic`, which also takes the fill. `Record::tap_count` is removed.

### Added

- `pixel-factor-service` 0.1: an HTTP API (challenge, enrol, verify, delete, health, version) over axum and SQLite, configured from the environment. Unknown user ids are indistinguishable from real ones in answers, timing and lockout. Lockout is exponential and expires. Development pepper only. `make serve` runs it locally and `make smoke` drives it end to end; CI runs the smoke test.
- Container image for the service (distroless, non-root, health check, JSON logs, clean shutdown on SIGTERM), scanned by Trivy and smoke-tested in CI. `docs/OPERATIONS.md` covers deployment, the running version, logs and alerts.
- `pixel-factor-core` 0.1: enrolment and verification of an ordered tap sequence with hexagonal tolerance, chained offset encryption and a single peppered verifier. Record format version 1, locked by a known-answer vector.
- Quality gates: strict lints, property tests, a fuzz target for `verify`, mutation testing, line coverage with a 98% floor, dependency policy with `cargo-deny`, and ShellCheck.
- CI: `make check` and `make coverage` on every pull request, and weekly advisories, mutation testing and fuzzing.
- Documentation: product principles, design spec, byte-level format notes, threat model and FAQ.
- `docs/HOW-IT-WORKS.md`: a plain-language explainer with geometry diagrams (generated SVGs) and Mermaid flowcharts, linked from the README.

### Dependencies

- argon2 0.6, hmac 0.13, sha2 0.11 and getrandom 0.4. The record format is unchanged.
