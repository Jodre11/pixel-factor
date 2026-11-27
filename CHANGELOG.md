# Changelog

Notable changes, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Nothing has been released yet; versions will follow [Semantic Versioning](https://semver.org/) once something is.

A change to the stored record format is always called out here, with the new `RECORD_VERSION`.

## Unreleased

### Changed

- **Record format version 2.** `enc_offsets` is always 124 bytes, padded with random fill, so a stored record no longer reveals how many taps it has. Verification tries any submitted count from 1 to 32 at the same cost. Version 1 records are rejected. `enrol_with_salt` is now `enrol_deterministic`, which also takes the fill. `Record::tap_count` is removed.

### Added

- `pixel-factor-core` 0.1: enrolment and verification of an ordered tap sequence with hexagonal tolerance, chained offset encryption and a single peppered verifier. Record format version 1, locked by a known-answer vector.
- Quality gates: strict lints, property tests, a fuzz target for `verify`, mutation testing, line coverage with a 98% floor, dependency policy with `cargo-deny`, and ShellCheck.
- CI: `make check` and `make coverage` on every pull request, and weekly advisories, mutation testing and fuzzing.
- Documentation: product principles, design spec, byte-level format notes, threat model and FAQ.

### Dependencies

- argon2 0.6, hmac 0.13, sha2 0.11 and getrandom 0.4. The record format is unchanged.
