# pixel-factor-core: format and implementation notes (v0.1)

Date: 2026-11-27. Byte-level detail for the Rust core. `spec.md` gives the design and the reasoning.

## Offsets use lattice coordinates

- Hexagon centres form a lattice with basis a1 = (sqrt(3) s, 0), a2 = (sqrt(3) s / 2, 1.5 s).
- A point is written as q * a1 + t * a2.
- Secret index = (floor q, floor t).
- Public offset = (frac q, frac t), each quantised to 16 bits.

The offset domain is a plain 65536 x 65536 square, so XOR with a 4-byte keystream is format preserving: every decryption is a valid offset. Verification subtracts the offset and rounds to the nearest hexagon centre, so the accepted region is the hexagon centred on the enrolled point.

An offset reveals position within a cell, not which cell.

An earlier draft stored offsets as positions inside a hexagon. That needed format-preserving encryption over an awkward set, and was dropped.

## Units

Coordinates are in units of image width. x runs 0 to 1, y runs 0 to aspect (height / width). Inputs are f64.

## Input limits

Enforced at enrolment (`InvalidPolicy`, `InvalidImageId`) and again when a record is read (`InvalidRecord`):

| Input | Limit | Why |
|---|---|---|
| r | 0.0001 up to, but not including, 0.25 | Below the minimum, cell indices could exceed an i32. |
| aspect | above 0, at most 16 | Bounds the cell index range. |
| image id | 1 to 255 bytes | Its length is part of the context. |
| taps | at most 32 | The index count is one byte. |

## Byte formats

All integers big-endian.

| Item | Encoding |
|---|---|
| Cell index | i32 q, i32 t (8 bytes) |
| Index list | u8 count, then each index |
| Offset | u16 u, u16 v (4 bytes) |
| Context | u32 length of image id, image id bytes, f64 bits of r, f64 bits of aspect |
| Salt | 16 random bytes |

## Derivations

Offset keystream for tap k (k >= 2):

```
HMAC-SHA256(pepper, "pfv1/offset" || 0x00 || salt || context || index_list(1..k-1))
```

First 4 bytes are XORed with the offset. The index list carries its count, so each position has its own key.

Verifier:

```
stretched = Argon2id(password = "pfv1/kdf" || 0x00 || context || index_list(1..n),
                     salt, 32 bytes out)
verifier  = HMAC-SHA256(pepper, "pfv1/final" || 0x00 || stretched)
```

The context binds a record to its image id, r, and aspect. Changing any of them makes verification fail.

## Record (JSON via serde)

`version`, `image_id`, `r`, `aspect`, `kdf {m_kib, t, p}`, `pepper_version`, `salt`, `first_offset`, `enc_offsets` (4 bytes per tap after the first), `verifier`. Byte fields are hex strings.

## Enrolment rules as implemented

- Tap count between `min_distinct` and `min_distinct + max_revisits` (default 5 to 6).
- Every tap inside the image.
- Consecutive taps at least 2s apart, else `ConsecutiveTooClose`.
- A tap inside the hexagon of an earlier distinct point is snapped to that point (a revisit).
- At least `min_distinct` distinct points, else `TooFewDistinct`.
- Distinct points closer than 4r produce a `PointsClose` warning.

## Verification behaviour

- Wrong tap count, out-of-bounds, or non-finite taps: `Ok(false)`.
- Offsets have no integrity check. Tampered or wrongly decrypted offsets give `Ok(false)` or, by chance, a match on that tap only. Never an error.
- `Err` is reserved for a malformed record or a pepper provider failure (including an unknown pepper version).
- Final comparison is constant time.

## Finding worth knowing

A wrongly decrypted offset still matches its tap roughly half the time, because any offset maps the tap to some cell. This does not weaken the scheme: the verifier covers all indices jointly, and a wrong earlier index already guarantees failure. It does mean offsets alone can never be used as a correctness signal, which is the intent.

## Measured

Release build, default Argon2id (64 MiB, 3 iterations): about 120 ms per enrolment or verification on the build machine. The pepper is called n times per operation (n - 1 keystreams plus the final HMAC).

## Tests

22 tests: geometry (20,000 random points: always accepted inside r, always rejected beyond s), ordering, tap count, bounds, pepper mismatch and version, record binding, revisit snapping and concealment, enrolment rules, JSON round trip, and a known-answer vector (`tests/vectors/kat1.json`).

## Not yet done

- Pepper rotation helper (re-wrap on successful login).
- Zeroising secrets in memory.
- Independent review. Unaudited.
- HTTP service, Keycloak plugin.
