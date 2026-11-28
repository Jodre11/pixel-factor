# pixel-factor

[![CI](https://github.com/Jodre11/pixel-factor/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Jodre11/pixel-factor/actions/workflows/ci.yml)
[![Licence: MIT OR Apache-2.0](https://img.shields.io/badge/licence-MIT%20OR%20Apache--2.0-blue)](#licence)

Sign in by tapping an ordered sequence of points on an image.

Each tap is accepted within a hexagonal tolerance of its enrolled position, so imprecise fingers still get in. The stored record is breach resistant: without a pepper held outside the database, it cannot be used to recover or guess the sequence.

Status: early and unaudited. Do not use to protect anything real yet.

**New here? Start with [How it works](docs/HOW-IT-WORKS.md)**, a plain-language tour with diagrams.

## Cryptographic review wanted

Nothing here has been independently reviewed. Before anyone relies on it, it needs people who know cryptography to try to break it. If that is you, please look.

Where to start:

- [docs/THREAT-MODEL.md](docs/THREAT-MODEL.md): what is claimed, against whom, and what is out of scope.
- [docs/spec.md](docs/spec.md) sections 5 to 8: enrolment, the stored record, chained offset encryption and verification.
- [docs/core-format.md](docs/core-format.md): every byte that is hashed or stored.
- `core/src/lib.rs` and `core/src/geometry.rs`: the implementation, about 750 lines.

The questions that matter most:

- Can a stolen record be used to test a guess, or part of one, without the pepper?
- Does anything reveal which tap in a sequence was wrong?
- Is the unauthenticated, chained offset encryption sound for what it is used for?

Report a break privately, as [SECURITY.md](SECURITY.md) describes. Open an issue for questions about the design or doubts that are not an attack. [docs/FAQ.md](docs/FAQ.md) answers the common ones.

## Who it is for

- **People signing in**, who want something easier to remember than a password and that works on any device.
- **Engineers adding a knowledge factor** to a login flow such as Keycloak.
- **Security reviewers**, who need to see exactly what is stored and what an attacker can do with it.

It is a knowledge factor, meant to be paired with a possession factor such as a passkey. It is not a sole factor. More in [docs/PRODUCT.md](docs/PRODUCT.md).

## Get started

```
make setup    # once, on a fresh checkout
make check    # format, lint, tests, docs
```

The Rust toolchain is pinned in `rust-toolchain.toml` and installed by rustup on first use. `make help` lists every command.

## Layout

| Path | What |
|---|---|
| `core/` | Rust library: geometry, enrolment, verification. No I/O. |
| `service/` | HTTP service around the core: axum and SQLite, with a container image. |
| `docs/` | How it works, product, design spec, byte-level formats, threat model and operations. |
| `keycloak/` | Keycloak authenticator plugin and a Docker demo: `make demo`. |
| `scripts/` | Check scripts used by the Makefile, the git hook and CI. |

## Read next

- [docs/HOW-IT-WORKS.md](docs/HOW-IT-WORKS.md): the design in plain language, with diagrams.
- [AGENTS.md](AGENTS.md): how to work in this repo, vocabulary and gotchas.
- [docs/PRODUCT.md](docs/PRODUCT.md): audience, principles and non-goals.
- [docs/spec.md](docs/spec.md): the design and its reasoning.
- [docs/core-format.md](docs/core-format.md): byte-level formats.
- [docs/THREAT-MODEL.md](docs/THREAT-MODEL.md): attackers, protections and known limits.
- [docs/FAQ.md](docs/FAQ.md): why hexagons, why not a zero-knowledge proof, how many bits.
- [docs/service.md](docs/service.md) and [docs/OPERATIONS.md](docs/OPERATIONS.md): running the HTTP service.
- [CHANGELOG.md](CHANGELOG.md): what has changed.
- [CONTRIBUTING.md](CONTRIBUTING.md): how changes are proposed and reviewed.
- [SECURITY.md](SECURITY.md): reporting a vulnerability.
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md): how we treat each other.

## Licence

Dual licensed under MIT or Apache-2.0, at your option. See `LICENSE-MIT` and `LICENSE-APACHE`.
