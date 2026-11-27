# Threat model

What pixel-factor protects, from whom, and where it stops. Drawn from [spec.md](spec.md); the byte-level detail is in [core-format.md](core-format.md). Early and unaudited: treat every claim here as a design intent to be checked, not an assurance.

## What is protected

| Asset | Where it lives | Why it matters |
|---|---|---|
| The tap sequence | Only in the user's memory | It is the secret. Everything else exists to avoid revealing it. |
| The pepper | A KMS or HSM that performs the HMAC | Every offline check of a guess needs it. |
| The record | The service's database | Safe to store only while the pepper stays separate. |

## Trust boundaries

```
Client (tap page)  --taps, in width units-->  Service (core)  --HMAC request-->  KMS (pepper)
                                                   |
                                               Record store
```

- The client is untrusted. It sends taps and nothing else; it never sees offsets, the pepper or a verification result beyond yes or no.
- The record store is assumed to leak eventually.
- The KMS is the one component whose compromise changes the outcome.

## Attackers

| Attacker has | Can do | Cannot do |
|---|---|---|
| The database only | Read every field of every record, including the first point's position within its cell. | Test a single guess offline. Each offset key and the verifier need the pepper. |
| The login endpoint only | Submit guesses online. | Learn which tap was wrong. The answer is one yes or no for the whole sequence. |
| The database and the pepper | Guess offline, starting with the most likely points on the image. | Nothing stops this. The sequence falls in time; Argon2id only slows it. Out of scope: protect the pepper. |
| A view of the user's screen or hand | Watch the sequence being entered. | Not addressed. See "Not addressed" below. |

## What the design does about it

| Property | Mechanism |
|---|---|
| No offline guessing without the pepper | The verifier is an HMAC under the pepper over an Argon2id hash of every cell index. |
| No per-tap signal from the record | One verifier covers the whole sequence. Offsets after the first are encrypted with keys chained through the earlier indices, with no authentication tag, so a wrong key still yields a plausible offset. |
| No per-tap signal from the endpoint | `verify` returns only match or no match. Anything wrong with the taps is a no match, never an error. The final comparison is constant time. |
| Revisits are hidden | Each position gets its own key, so a repeated point does not repeat in the record. |
| A record cannot be moved to another image or tolerance | The image id, r and aspect are bound into every key and the verifier. |
| Pepper rotation | Each record names the pepper version it uses. |

## Known limits

- **The secret space is small.** About 44 theoretical bits at the default tolerance, and less in practice, because people favour the same points on an image. The working assumption is a loss of a third to a half. The pepper and rate limiting carry the security, not the key derivation function.
- **The first offset is in the clear.** It shows where the first point sits within its cell, never which cell.
- **Hotspots are estimated, not measured.** No user study has been run.
- **Unaudited.** No independent cryptographic review has been done.

## What the operator must provide

The core cannot enforce these. An integration that skips one loses the protection above.

- The pepper in a KMS or HSM that performs the HMAC, never in application configuration.
- Rate limiting and lockout on the login endpoint.
- Transport security between the client, the service and the KMS.
- A possession factor alongside this one. It is a knowledge factor, not a sole factor.
- A reset path that proves identity another way before re-enrolment.

## Not addressed

- **Shoulder surfing and recording.** Anyone who sees the taps learns the sequence, as with a PIN.
- **Smudge marks** on a touchscreen may hint at tapped points.
- **A compromised client or device.** Malware that reads the taps reads the secret.
- **Memory disclosure on the server.** Secrets are not yet zeroised in memory (spec section 19).
- **Side channels beyond the constant-time comparison** have not been analysed.

## Reporting

Anything that breaks a row in "What the design does about it" is a vulnerability. Report it privately: see [SECURITY.md](../SECURITY.md).
