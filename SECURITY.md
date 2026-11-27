# Security

This project is early and unaudited. Do not use it to protect real accounts yet.

## Reporting a vulnerability

Report privately through GitHub's "Report a vulnerability" button under the Security tab of this repository. Do not open a public issue.

Include what you found, how to reproduce it, and what an attacker gains.

## What counts

Anything that breaks a principle in [docs/PRODUCT.md](docs/PRODUCT.md), in particular:

- a way to test guesses against a stored record without the pepper
- anything that reveals which tap in a sequence was wrong
- a tap outside tolerance being accepted, or the reverse

The small size of the secret space is a known and documented limit, not a vulnerability. See `docs/spec.md`.
