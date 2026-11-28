# Pixel Based Factor Verification: Spec v0.9

Status: draft, 2026-11-27. Core library built (v0.1, unaudited).

Changes in v0.9: the possession factor (passkey or TOTP) comes before the tap step, not after it, so only its holder can record a failed attempt and a stranger cannot use the lockout to keep a user out. A password is never the first step: password and taps are both knowledge factors.

Changes in v0.8: sequences may be 5 to 8 taps. Revisits are no longer limited to one; any number is allowed within the tap count, provided there are at least 5 distinct points and no tap repeats the point just before it. The record format is unchanged.

Changes in v0.7: the stored record no longer reveals the tap count. Offsets are padded to a fixed 31 slots with random fill (record format version 2).

Changes in v0.6: section 18 reduced to public sources and prior art.

Changes in v0.5: sections 4 to 8 rewritten to match the Rust core. Offsets now use lattice coordinates. Record binding, units, and error behaviour made explicit. Build status added. Byte-level detail lives in `core-format.md`.

Changes in v0.3: Argon2id parameters, image policy, recovery model, proximity warning, Rust core as an HTTP service, Keycloak authenticator as first integration.

Changes in v0.2: single enrolment for all devices at a universal tolerance (r = 2.2% of image width, n = 5).

## 1. Purpose

A knowledge factor where the secret is an ordered sequence of n taps on a 2D image. Each tap is accepted within a tolerance of its enrolled position. A stolen database must not allow the sequence to be recovered.

"Zero knowledge" here means breach resistance of the stored verifier. It is not a zero-knowledge proof protocol.

## 2. Threat model

| Attacker has | Outcome |
|---|---|
| Database only | Cannot guess offline. Pepper is required for every check. |
| Login endpoint only | Limited by rate limiting and lockout. |
| Database and pepper | Sequence falls to hotspot-ordered guessing. Out of scope; protect the pepper. |

No scheme makes the secret irreversible outright. The secret space is small (about 44 theoretical bits), so the pepper and rate limiting carry the security, not the KDF alone.

## 3. Decisions made

- Tolerance by discretisation, with a per-point shifted grid (centred discretisation). Users tap anywhere; nothing snaps to a visible grid.
- Hexagonal cells, pointy-top.
- Tap order matters.
- One joint hash over the whole sequence. Never a hash per point.
- Per-user salt, plus a pepper held outside the database.
- Sequences of 5 to 8 taps, with at least 5 distinct points. Revisits allowed, any number, if not adjacent in the sequence.
- Offsets after the first are encrypted with a key chained through the sequence.
- Offsets are expressed in lattice coordinates, so their domain is a plain square.
- Single enrolment. One record works on every device: enrol on desktop, log in on a phone, or the reverse.
- Universal tolerance r = 2.2% of image width, sized for touch with about 3x zoom.
- Argon2id: 64 MiB, 3 iterations, parallelism 1.
- Curated image set for the MVP. No user uploads yet.
- No recovery of a sequence. Reset means re-enrolment after proving identity another way.
- Core library in Rust, deployed as an HTTP service.
- First integration: Keycloak, via a thin authenticator plugin.
- Free and open source, dual licensed MIT or Apache-2.0.

## 4. Geometry

Units: all coordinates are in units of image width. x runs 0 to 1, y runs 0 to aspect, where aspect = height / width. Screen pixels never reach the core.

- r: guaranteed tolerance (hexagon inradius)
- s = 2r / sqrt(3): hexagon circumradius, worst-case accepted distance (1.155r)
- Cell area: 2 * sqrt(3) * r^2 (about 3.46 r^2)

Hexagon centres form a lattice with basis vectors

```
a1 = (sqrt(3) s, 0)
a2 = (sqrt(3) s / 2, 1.5 s)
```

Lattice coordinates (q, t) of a point (x, y), such that point = q a1 + t a2:

```
t = y / (1.5 s)
q = x / (sqrt(3) s) - t / 2
```

Nearest hexagon centre, by cube rounding:

```
a = q, c = t, b = -a - c
round a, b, c; recompute the one with the
largest rounding error so that a + b + c = 0
centre = (a, c)
```

This runs only in the Rust core. A single implementation avoids floating point differences at cell edges.

## 5. Enrolment

Each point splits into a secret index and a public offset:

- index = (floor q, floor t), two signed 32-bit integers
- offset = (frac q, frac t), each quantised to 16 bits

By construction, point minus offset lands on a lattice point, which is a hexagon centre. The offset reveals where the point sits within a cell, never which cell.

Steps:

1. Check tap count is between min_distinct and max_taps (default 5 to 8).
2. Check every tap is inside the image.
3. Reject if any two consecutive taps are less than 2s apart.
4. If a tap falls inside the hexagon of an earlier distinct point, snap it to that exact point. This is a revisit.
5. Reject if there are fewer than min_distinct distinct points.
6. Warn (do not reject) for distinct points closer than 4r. Valid, but fiddly on a phone.
7. Split each point, encrypt offsets 2 to n (section 7), compute the verifier (section 6).

## 6. Stored record

| Field | Notes |
|---|---|
| version | Format version, currently 2 |
| image_id | Which image |
| r, aspect | Geometry |
| kdf | Argon2id settings |
| pepper_version | For rotation |
| salt | 16 random bytes, per user |
| first_offset | 4 bytes, cleartext |
| enc_offsets | 124 bytes: 31 slots of 4. Encrypted offsets of taps 2 to n, then random fill |
| verifier | 32 bytes |

Verifier:

```
stretched = Argon2id("pfv1/kdf" || 0x00 || context || index_list, salt)
verifier  = HMAC-SHA256(pepper, "pfv1/final" || 0x00 || stretched)
```

- index_list is a count byte followed by each index as two big-endian i32, in tap order.
- context is the image id, r, and aspect. It binds the record to its parameters: changing any of them makes verification fail.
- Nothing device specific is stored.
- The tap count is not stored. Every record is the same size, and without the pepper the fill cannot be told apart from an encrypted offset. The documented minimum (section 9) is public; how many taps a particular user chose is not.

## 7. Chained offset encryption

Keystream for the offset of tap k, for k = 2 to n:

```
HMAC-SHA256(pepper, "pfv1/offset" || 0x00 || salt || context || index_list(1..k-1))
```

The first 4 bytes are XORed with the 4-byte offset.

Properties:

- The index list carries its own count, so each position gets an unrelated key, even for a repeated point.
- Format preserving for free: the offset domain is a 65536 by 65536 square, so every decryption is a valid offset.
- No authentication tag. A wrong key must yield a plausible offset, never an error, or each tap becomes separately checkable.
- No intermediate hashes stored.

Effect: a stolen record does not show which taps revisit a point, and the within-cell leak is limited to the first point.

Note: a wrongly decrypted offset still matches its own tap about half the time, since any offset maps a tap to some cell. This is harmless. The verifier covers all indices jointly, and a wrong earlier index already guarantees failure. Offsets can never serve as a correctness signal, which is the intent.

## 8. Verification

The verifier does not know n. It treats the number of taps submitted, m, as the count, for any m from 1 to 32.

1. Tap 1: subtract first_offset, round to the nearest centre, giving index'_1.
2. For k = 2 to m: derive the keystream from the indices so far, decrypt offset_k, subtract it from tap k, round, giving index'_k.
3. Compute the verifier over index'_1 to index'_m and compare in constant time. The index list starts with its count, so m other than n fails here like any other wrong sequence.

Outcomes:

- Match: every tap inside its hexagon, in order.
- No match: anything else, including a wrong tap count, taps outside the image, or non-finite coordinates. All or nothing, with no indication of which tap was wrong.
- Error: reserved for a malformed record or a pepper provider failure, such as an unknown pepper version. Never caused by the taps.

The pepper is called m times per verification (m - 1 keystreams and one final HMAC) and n times per enrolment. The cost of a verification depends on the count submitted, never on the count enrolled. Only an empty submission, one over 32 taps, or a tap outside the image returns early, and none of those depends on the record.

## 9. Entropy and parameters

Bits per point = log2(A / (3.46 r^2)), where A is image area. For a 4:3 image this is log2(0.217 * (W/r)^2).

Chosen: r = 2.2% of image width (W/r = 45), against a target of 40 theoretical bits.

- Cells per image: about 440
- Bits per point: 8.8
- n = 5 distinct points: about 44 theoretical bits
- Up to 8 taps, so a user may add distinct points (about 8.8 bits each) or revisits for more strength. The record does not show which they chose, or how many.

Reference table for other tolerances:

| r (of width) | W/r | Phone zoom needed | Bits per point | Distinct points for 40 bits |
|---|---|---|---|---|
| 6.7% | 15 | none | 5.6 | 8 |
| 3.3% | 30 | 2x | 7.6 | 6 |
| 2.2% | 45 | 3x | 8.8 | 5 |
| 1% | 100 | 6 to 7x | 11.1 | 4 |

Notes:

- Working assumption: hotspot clustering costs a third to a half of the theoretical bits.
- n and r are fixed at enrolment and cannot vary at login.
- Rejected: storing a fine and a coarse verifier from one enrolment so desktop needs fewer taps. The attacker targets the coarse one, so it gains nothing.

## 10. Key derivation

Argon2id with 64 MiB memory, 3 iterations, parallelism 1. Measured at about 120 ms per enrolment or verification in a release build.

This is defence in depth only. With the pepper safe it is redundant; with the pepper lost it merely slows a 44-bit search.

## 11. Images

- Curated set for the MVP.
- Each image vetted for at least 30 or so distinct tappable features spread across the frame.
- No faces, text, or single dominant subjects.
- Image id stored in the record and bound into the verifier. An image in use is never changed or re-cropped.

Later options: user uploads (more memorable, unvetted), and viewport-constrained enrolment, where each point must be chosen inside a randomly placed viewport to spread choices out.

## 12. Client rules

- Same framing on every device: the whole image, letterboxed, never cropped to fit.
- Zoom is a client rendering matter only. Expected on phones (about 3x), optional on desktop.
- The client reports taps in units of image width.
- The client never receives offsets or the pepper, and never runs verification.

## 13. Operational rules

- Pepper lives in a KMS or HSM that performs the HMAC. Never in app config.
- Rate limit and lock out on the login endpoint.
- Pepper rotation: re-wrap on next successful login, tracked by pepper_version.

## 14. Recovery

None, by design: nothing stored can reveal the sequence. A reset requires proving identity another way (a full login with the possession factor, or an admin; the possession factor alone is not enough) and then enrolling again. Same model as a password reset.

## 15. Positioning

This is a knowledge factor, the same category as a password. Pairing it with a password is not multi-factor. It fits as a password or PIN replacement, paired with a possession factor (passkey, TOTP).

## 16. Architecture

```
Browser (tap page)
   |  taps, in width units
Keycloak authenticator plugin (Java, glue only)
   |  HTTP + JSON
Rust service: /enrol, /verify
   |              |
Record store     KMS (pepper HMAC)
```

- Rust core library: geometry, chained offsets, Argon2id, verifier. No I/O.
- Rust HTTP service: wraps the core, owns the records, calls the KMS through the core's PepperProvider interface (in-memory key in dev, KMS in prod).
- Adapters speak HTTP to the service. No FFI bindings needed for the MVP.
- The Keycloak plugin renders the tap page and calls the service. It contains no crypto. It replaces the password form. A passkey or TOTP comes first and the tap step second, so only someone holding the possession factor can record a failed attempt, and a stranger cannot use the lockout to keep a user out.

Rejected for now: loading Rust into Keycloak's JVM (JNI or a Java WASM runtime). It complicates deployment.

## 17. Build order and status

1. Rust core library, with test vectors. DONE: `pixel-factor-core` v0.1. Example, property and known-answer tests, fuzzing and mutation testing. Unaudited.
2. Rust HTTP service. DONE: `pixel-factor-service`, axum and SQLite, with lockout and a container image. See `service.md`.
3. Keycloak authenticator plugin: first working demo, Keycloak in Docker. DONE: `pixel-factor-keycloak` and `make demo`. The tap step runs only after the possession factor (section 16). See `keycloak.md`.
4. OIDC host reusing the same service, for Entra ID external MFA and Okta's IdP authenticator, subject to how each classifies the factor type. BLOCKED: neither accepts a knowledge factor as a second factor (checked 28 November 2026).
   - Entra ID external authentication methods accept only possession `amr` values (`fido`, `otp`, `hwk`, `sms` and others) or inherence values (`face`, `fpt` and others), and "only methods with a different type can satisfy the second factor requirement". No knowledge value (`pwd`, `pin`, `kba`) is accepted. Source: Microsoft Learn, "Microsoft Entra External MFA Method Provider Reference".
   - Okta's IdP authenticator "is a possession factor and verifies user presence". The type is fixed. Source: Okta Help, "Configure the IdP authenticator".
   - Both put the tap step after the platform's own first factor, usually a password, so the possession-first rule in section 16 cannot hold.

   The host could pass only by reporting a possession factor it does not provide, which breaks principle 3 in `PRODUCT.md`. An honest host would have to run its own possession factor (passkey or OTP) before the tap step and report that. That is a different product and is not planned.

Test early: enrol on desktop, log in on a phone.

## 18. Sources and prior art

Every technique used here comes from published work. This is not legal advice.

- Click-point graphical passwords: Blonder, US patent 5,559,961 (1996, expired); PassPoints (Wiedenbeck et al., 2005).
- Tolerance by discretisation: robust discretisation (Birget, Hong, Memon, 2006); centred discretisation (Chiasson, Srinivasan, Biddle, van Oorschot, 2008).
- Hexagonal grids: standard axial and cube coordinate rounding.
- Key derivation: Argon2id, RFC 9106.
- Salt, pepper, and HMAC: standard practice.

Related prior art, checked 27 November 2026:

- GB2482886, "Improved security system for controlling local and remote access", filed 2010, published as an application in 2012. Never granted; terminated 2016. Patent family of one. Discloses a sequence of points on an image with a tolerance region around each.

This was a targeted lookup, not a formal clearance search.

## 19. Open questions

- Hotspot mitigation beyond image vetting.
- Which KMS for the first deployment.
- Pepper rotation helper in the core.
- Zeroising secrets in memory.
- Independent security review before any real use.
