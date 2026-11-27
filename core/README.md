# pixel-factor-core

Breach-resistant verification of an ordered sequence of taps on an image, with a hexagonal tolerance around each point.

A stolen record cannot be used to recover or guess the sequence without a pepper held outside the database.

Status: v0.1, unaudited. Do not use to protect anything real yet.

## Use

```rust
use pixel_factor_core::geometry::Point;
use pixel_factor_core::{enrol, verify, Policy, StaticPepper};

// Coordinates are in units of image width: x in 0..=1, y in 0..=aspect.
let taps = [
    Point::new(0.10, 0.10),
    Point::new(0.80, 0.20),
    Point::new(0.30, 0.60),
    Point::new(0.55, 0.35),
    Point::new(0.90, 0.70),
];
let pepper = StaticPepper::new(1, b"dev only".to_vec()); // use a KMS in production
let policy = Policy::standard(0.75);                     // 4:3 image

let enrolment = enrol(&taps, "image-42", &policy, &pepper)?;
// store enrolment.record (serde), show enrolment.warnings to the user

let ok = verify(&taps, &enrolment.record, &pepper)?;
```

## What the caller must provide

- A `PepperProvider` backed by a KMS or HSM. `StaticPepper` is for development.
- Rate limiting and lockout on the login path. The secret space is small.
- Record storage.
- A client that shows the whole image, never cropped, and reports taps in width units.

## Defaults

| Setting | Value |
|---|---|
| Tolerance r | 2.2% of image width |
| Distinct points | at least 5 |
| Taps | 5 to 8. Revisits are unlimited within that, but never straight after the point they repeat. |
| Argon2id | 64 MiB, 3 iterations, parallelism 1 |

## Test

```
cargo test
cargo run --release --example timing
```

`tests/vectors/kat2.json` is a known-answer vector. If it changes, the record format has changed.

## Licence

MIT or Apache-2.0, at your option.
