# Keycloak authenticator plugin: design

Status: design, not built. This is step 3 of the build order in [spec.md](spec.md) section 17: the first working demo, with Keycloak in Docker. The plugin is glue: it shows the tap page, calls the service and records the outcome. It holds no secret and does no geometry or cryptography (spec section 16, principle 5 in [PRODUCT.md](PRODUCT.md)).

## Goals

- A Keycloak user can enrol a tap sequence and then sign in with a possession factor followed by the tap step.
- The plugin refuses to run the tap step unless a possession factor has already succeeded in the same login. This is what stops a stranger using the service's lockout to keep a user out ([service.md](service.md), "Lockout as denial of service").
- One command brings up Keycloak, the service and a demo realm on a laptop.

## Non-goals for this step

- Production packaging: a KMS pepper, mutual TLS between Keycloak and the service, high availability. The service does not have these yet either.
- A curated image library. The demo ships one generated image.
- A browser-driven end-to-end test. Planned as the next step; see "Testing".

## Versions

| Component | Choice | Why |
|---|---|---|
| Keycloak | 26.8.2, the latest release | The SPI used here is stable across 26.x. |
| Java | 21 | The JDK Keycloak 26 runs on. |
| Build | Maven, run inside a pinned `maven` container | No JDK is needed on the host, matching how the service image is built. |
| Runtime dependencies | None beyond what Keycloak provides | `java.net.http` for HTTP, Keycloak's own Jackson for JSON. Nothing extra to vet or licence. |

## Components

All in one provider jar, `keycloak/` at the repository root.

| Piece | Keycloak SPI | Job |
|---|---|---|
| `PixelFactorAuthenticator` and its factory | `Authenticator` | The tap step at login. |
| `PixelFactorEnrolAction` and its factory | `RequiredActionProvider` | Enrolment and re-enrolment. |
| `PixelFactorCredentialProvider` | `CredentialProvider` | A marker credential, so Keycloak knows who is enrolled and the account console can show and remove it. |
| `PixelFactorUserListener` | `ProviderEventListener` | Deletes the service record when a Keycloak user is deleted. |
| `ServiceClient` | none | The four calls to the service. |
| `pixel-factor-tap.ftl` and `tap.js` | theme resources in the jar | The tap page. |

### The marker credential

The credential stores no secret: its secret data is empty and its credential data holds only the image id. The record lives in the service. The marker exists so that `configuredFor` can answer "is this user enrolled?" without a network call, so the credential shows in the account console, and so removing it (by the user or an admin) deletes the record at the service.

### User ids

The plugin sends Keycloak's internal user id (a UUID), never the username. A rename does not orphan the record, and the service's logs hold no username. UUIDs are unique across realms, so one service can serve several realms.

## Configuration

Server-wide, through Keycloak's SPI configuration, so the API token never sits in the realm database or the admin console:

| Variable | Meaning |
|---|---|
| `KC_SPI_AUTHENTICATOR_PIXEL_FACTOR_SERVICE_URL` | Base URL of the service. |
| `KC_SPI_AUTHENTICATOR_PIXEL_FACTOR_API_TOKEN` | The service's `PF_API_TOKEN`. |
| `KC_SPI_AUTHENTICATOR_PIXEL_FACTOR_POSSESSION_AUTHENTICATORS` | Provider ids that count as a possession factor. Default `webauthn-authenticator,webauthn-authenticator-passwordless,auth-otp-form`. |
| `KC_SPI_AUTHENTICATOR_PIXEL_FACTOR_TIMEOUT_MS` | Timeout for each service call. Default 5000. |

Missing URL or token: the factory fails at start, so a misconfigured Keycloak does not come up half working.

## Login: the tap step

The realm's browser flow places the tap step after a possession factor, in place of the password form:

```
Cookie                      ALTERNATIVE
Forms                       ALTERNATIVE
  Username form             REQUIRED
  OTP form or WebAuthn      REQUIRED
  Pixel factor tap step     REQUIRED
```

`authenticate`:

1. **Possession gate.** Look through the login's execution statuses for an execution whose provider id is in the possession list and whose status is `SUCCESS`. If there is none, fail the flow with `ACCESS_DENIED`, log a misconfiguration at error level, and never call the service. Fail closed: a flow edited to put the tap step first stops working rather than becoming abusable.
2. **Challenge.** `POST /v1/challenge` with the user id, then render the tap page with that image.

`action` (the form post):

1. Parse the taps (see "The tap page"). Anything unparseable is treated as an empty sequence, which the service answers with no match. The plugin never rejects taps on its own, so it adds no signal the service does not give.
2. `POST /v1/verify`.
3. `match: true`: `context.success()`.
4. `match: false, locked: false`: record a failed login event (`INVALID_USER_CREDENTIALS`) so Keycloak's own brute-force detection counts it, and show the tap page again with "That sequence did not match."
5. `locked: true`: the same event, and "Too many attempts. Try again later." Telling the user is safe: they have already passed the possession factor, and the service sets the flag for unknown ids too.
6. Service unreachable, timed out or `5xx`: fail with `INTERNAL_ERROR` and a generic page. Never fall through to success.

`configuredFor` is true when the user has the marker credential. A user without one gets the enrolment required action and must finish it before the login completes, so an enrolled-looking session never exists without a record.

## Enrolment

Run as a required action, either for a user who has never enrolled (set by `configuredFor`) or on request: the user from the account console after a full login, or an admin. Re-enrolment therefore needs a full login or an admin; the possession factor alone cannot reset the tap factor.

The service replaces a record as soon as `/v1/enrol` succeeds, and offers no staging. To make sure the user can repeat their sequence before it replaces the real one, enrolment uses a second, staging id, `pending:<user id>`:

1. `POST /v1/challenge` for the real id, to get the image. Show it.
2. First entry: `POST /v1/enrol` under the staging id. A `422` shows its reason and asks again. Any warnings ("taps 2 and 3 are close together") are shown, with a choice to continue or start again.
3. Confirmation entry: `POST /v1/verify` against the staging id. No match: "That did not match your first entry", back to step 2.
4. Match: `POST /v1/enrol` under the real id with the confirmation taps, then `DELETE` the staging id, then write the marker credential. The real record is built from the confirmation entry, which lies within tolerance of the first.

The taps are never held in Keycloak between requests: each request carries its own entry, and everything kept between steps is in the service. Each enrolment costs three Argon2id runs, about 0.4 seconds in all.

A user who abandons enrolment keeps their old record and marker untouched; only the staging id is left, and the next attempt replaces it.

## The tap page

Follows spec section 12.

- The whole image, letterboxed, never cropped. Pinch zoom is allowed on phones.
- Each tap is converted to units of image width: x and y divided by the rendered width, so y runs 0 to the aspect.
- The taps of the current entry are marked with numbered dots, with Undo and Clear. These are the user's own input on their own screen, like a password field's dots; nothing about earlier entries or the enrolled points is ever shown (non-goal "No hints").
- **Fixed-length submission.** The form field always holds 8 coordinate pairs, each formatted to a fixed width, with unused pairs as a fixed filler the plugin strips. The request size is the same for every sequence, so a network observer cannot count taps from it.
- The page sends taps and nothing else; it never receives offsets, records or the pepper.

The script is plain JavaScript with no external libraries, served from the jar, under Keycloak's default content security policy.

## Images

Keycloak and the service must agree on image ids and aspects. The jar carries image files at `resources/img/<image id>.<ext>` in the theme resources; the service's `PF_IMAGES` lists the same ids. A challenge for an id the jar does not have is a deployment error: fail with `INTERNAL_ERROR` and log it.

The demo ships one image, `demo`, aspect 0.75, drawn by a script in the style of `docs/images/make_diagrams.py`: an abstract scene with more than 30 distinct features, no faces and no text (spec section 11). Real curated images, and their licensing, are a later decision.

## Demo

A `docker compose` file in `keycloak/demo/` runs:

- `keycloak`: the official Keycloak image pinned by digest, in development mode, with the plugin jar mounted and a realm imported from `realm.json`.
- `service`: the image from the repository `Dockerfile`, with development settings and a fresh volume.

The realm has the flow above with OTP as the possession factor, and one user who sets up OTP and then enrols taps on first login. Only Keycloak's port is published; the service is reachable only from Keycloak on the compose network, as [OPERATIONS.md](OPERATIONS.md) expects.

## Commands

New Makefile targets, all running Maven inside the pinned container:

| Target | Does |
|---|---|
| `plugin` | Build the jar. |
| `plugin-check` | Format check, compile with warnings as errors, unit tests. |
| `demo` | Build the jar and the service image, start the compose stack, print the login URL. |
| `demo-smoke` | Start the stack, check that Keycloak loaded the provider and imported the realm, check that the service answers from inside the network, stop it. |

`make check` stays Rust-only and offline. Maven needs the network the first time it fetches Keycloak's jars, which would break that promise; the plugin targets are separate, and CI runs `plugin-check` and `demo-smoke` on every pull request. Dependabot gains the `maven` and `docker-compose` ecosystems.

## Testing

- **Unit tests** (JUnit 5, Mockito for Keycloak's context objects): the possession gate passes only on a listed provider with `SUCCESS` and fails closed otherwise; every service outcome maps to the right flow result; a service failure never yields success; the tap parser turns any bad input into an empty sequence; the enrolment steps call the service in the order above and write the marker only after the final enrol.
- **`ServiceClient` against a real service**: a test starts the service binary on a free port, as `make smoke` does, and drives the four calls.
- **Demo smoke**, as above.
- **Next step, not in this one**: a Playwright test that logs in through the demo with a generated OTP and taps the image.

## Assumptions

Recorded rather than asked; each can be revisited.

1. Maven over Gradle, built in a container rather than on the host.
2. Plugin targets are kept out of `make check` to keep it offline.
3. The real record is built from the confirmation entry, not the first.
4. Re-enrolment needs a full login or an admin, not just the possession factor.
5. The demo uses OTP rather than a passkey, because it can be scripted later.
6. One generated demo image; curated photographs come later.

## Docs to update when this lands

- `AGENTS.md` and `README.md`: the plugin moves from "planned" to the layout and commands tables.
- `PRODUCT.md`: the "Easy to adopt" row, which still says the service does not exist.
- `spec.md` section 17: step 3 status.
- `HOW-IT-WORKS.md`: the plugin is no longer "planned, not built yet".
- `service.md`: the lockout paragraph's "the Keycloak plugin will enforce that order".
