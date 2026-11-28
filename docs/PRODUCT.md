# Product: who this is for and why

Use this to judge whether a change is worth making, not just how to make it.

## The problem

Passwords are hard to remember and easy to leak. Picture-based secrets are easier to recall, but the usual way of allowing for imprecise taps means storing something that reveals the secret when the database is stolen.

pixel-factor exists to show that a tap-sequence secret can tolerate imprecise input and still survive a database breach.

## Who it serves

**People signing in.** They want to get into their account quickly, on whatever device is to hand, without remembering a string of characters. They are imprecise: fingers are blunt, and the device they log in on is often not the one they enrolled on. They are not security experts and should never need to be.

**Engineers adding it to a login flow.** They run an identity system such as Keycloak and want a knowledge factor they can drop in. They need a small API, clear rules about what they must provide (a pepper in a KMS, rate limiting), and honest limits so they do not deploy it where it does not belong.

**Security reviewers.** They need to see exactly what is stored, what an attacker with the database can and cannot do, and where the scheme's limits are. They are served by plain documentation and a small, readable core.

## Principles

In priority order. When two conflict, the higher one wins.

1. **A stolen database must not reveal the sequence.** No change may give an attacker with the records a way to test guesses without the pepper.
2. **All or nothing.** Verification says yes or no for the whole sequence. Nothing may reveal which tap was wrong, in a response, an error, a log line or a timing difference.
3. **Honest about limits.** The secret space is small. Documentation states the real numbers and never oversells. If a feature makes the system look stronger than it is, it does not ship.
4. **Tolerant of real people.** A tap within tolerance always works, on any device, in either direction between desktop and phone.
5. **One implementation of the maths.** Geometry and formats live in the core only, so behaviour cannot drift between hosts.
6. **Easy to adopt.** Integrators should need to supply as little as possible, and what they must supply is stated plainly.

## Non-goals

- **Not a sole factor.** It is a knowledge factor to pair with a possession factor. Do not add features that position it as sufficient alone.
- **Not a zero-knowledge proof.** "Zero knowledge" in this project's history means breach resistance of the stored record.
- **No recovery of a sequence.** Nothing stored can reveal it. A reset means enrolling again.
- **No per-device enrolment.** One enrolment works everywhere.
- **Not biometric.** It does not measure how someone taps, only where.
- **No hints.** Nothing that helps a user remember also helps an attacker, so no partial feedback and no reminders of earlier points.

## How to use this document

- A request that conflicts with a principle or lands in a non-goal should be challenged before any code is written.
- A change that trades a higher principle for a lower one needs a stated reason in the pull request.
- The pull request template asks which principle a change serves. Answer it.

## Where the product stands

| Promise | State |
|---|---|
| Stolen database does not reveal the sequence | Designed and implemented in the core. Unaudited. |
| All or nothing | Implemented and tested in the core. |
| Tolerant across devices | Implemented in the core. Not yet tested with real users or a real client. |
| Easy to adopt | A Keycloak plugin and a one-command Docker demo exist. No production pepper (KMS) yet, and no other identity system. |
| Honest about limits | Entropy figures are in `spec.md`. Hotspot effects are estimated, not measured. |
