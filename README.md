# pixel-factor

A knowledge factor based on an ordered sequence of taps on an image.

Each tap is accepted within a hexagonal tolerance of its enrolled position. The stored record is breach resistant: without a pepper held outside the database, it cannot be used to recover or guess the sequence.

Status: early and unaudited. Do not use to protect anything real yet.

## Layout

| Path | What |
|---|---|
| `core/` | Rust library: geometry, enrolment, verification. No I/O. |
| `docs/` | Design spec and byte-level format notes. |

Planned: an HTTP service around the core, and a Keycloak authenticator plugin.

## Build and test

```
cargo test
cargo run --release -p pixel-factor-core --example timing
```

## Licence

Dual licensed under MIT or Apache-2.0, at your option. See `LICENSE-MIT` and `LICENSE-APACHE`.
