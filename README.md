# pixel-factor

Sign in by tapping an ordered sequence of points on an image.

Each tap is accepted within a hexagonal tolerance of its enrolled position, so imprecise fingers still get in. The stored record is breach resistant: without a pepper held outside the database, it cannot be used to recover or guess the sequence.

Status: early and unaudited. Do not use to protect anything real yet.

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
| `docs/` | Product, design spec and byte-level format notes. |
| `scripts/` | Check scripts used by the Makefile, the git hook and CI. |

Planned: an HTTP service around the core, and a Keycloak authenticator plugin.

## Read next

- [AGENTS.md](AGENTS.md): how to work in this repo, vocabulary and gotchas.
- [docs/PRODUCT.md](docs/PRODUCT.md): audience, principles and non-goals.
- [docs/spec.md](docs/spec.md): the design and its reasoning.
- [docs/core-format.md](docs/core-format.md): byte-level formats.
- [CONTRIBUTING.md](CONTRIBUTING.md): how changes are proposed and reviewed.
- [SECURITY.md](SECURITY.md): reporting a vulnerability.

## Licence

Dual licensed under MIT or Apache-2.0, at your option. See `LICENSE-MIT` and `LICENSE-APACHE`.
