# FAQ

Short answers to the questions reviewers ask first. The reasoning is in [spec.md](spec.md) and [THREAT-MODEL.md](THREAT-MODEL.md).

## Why hexagons?

The design needs cells that tile the image, so every tap lands in exactly one, and each cell should be as close to a circle as possible. A tap within distance r of the enrolled point must always be accepted, and a tap much further away should not be.

Of the shapes that tile the plane, hexagons come closest. For the same guaranteed tolerance r (the inradius):

| Cell | Area | Worst-case accepted distance |
|---|---|---|
| Square | 4 r² | 1.414 r |
| Hexagon | 3.46 r² | 1.155 r |

Smaller cells mean more cells per image, so more bits per point at the same tolerance. A smaller worst case means fewer taps accepted that the user did not aim for. Rounding to the nearest hexagon is a standard, exact integer operation (cube rounding), and it lives in one place in the core.

## Why not a zero-knowledge proof?

Because it solves a different problem. A zero-knowledge proof, or a password-authenticated key exchange, stops the server from learning the secret during login. The threat here is a stolen database.

The secret is small, about 44 bits. Any stored record that can be checked without a server-held secret falls to brute force, whatever protocol created it. What protects the record is the pepper, held in a KMS. A proof protocol would also move the tolerance geometry into every client, where it could round differently at cell edges.

"Zero knowledge" in this project's history meant breach resistance of the stored record, not a proof protocol. See the non-goals in [PRODUCT.md](PRODUCT.md).

## How many bits is it?

About 44 theoretical bits at the defaults: a tolerance of 2.2% of image width gives about 440 cells on a 4:3 image, or 8.8 bits per point, and five distinct points are required.

The real figure is lower, because people favour the same points on an image. The working assumption is a loss of a third to a half, which leaves roughly 22 to 29 bits. That is an estimate; no user study has been run.

That is comparable to a short PIN or a weak password, not a strong one. It is why this is a knowledge factor to pair with a passkey or TOTP, never a sole factor, and why the pepper and rate limiting carry the security. The table in spec section 9 gives figures for other tolerances.

## Why is there no recovery?

Because nothing stored can reveal the sequence, and that is the point. A recovery mechanism would need something that does, and an attacker would use it first.

A user who forgets the sequence proves who they are another way, through their possession factor or an administrator, and enrols again. It works like a password reset.

## Why does a wrongly decrypted offset still match its tap half the time?

Any offset maps a tap to some cell, so a wrong one sometimes gives the same cell as the right one. This is intended. The offsets carry no authentication tag, so they can never act as a signal that a tap was right. Only the single verifier over the whole sequence decides.
