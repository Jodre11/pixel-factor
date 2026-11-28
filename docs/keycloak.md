# Keycloak authenticator plugin: design

Status: built; `make demo` runs it. This is step 3 of the build order in [spec.md](spec.md) section 17: the first working demo, with Keycloak in Docker. The plugin is glue: it shows the tap page, calls the service and records the outcome. It holds no secret and does no geometry or cryptography (spec section 16, principle 5 in [PRODUCT.md](PRODUCT.md)).

## Goals

- A Keycloak user can enrol a tap sequence and then sign in with a possession factor followed by the tap step.
- The plugin refuses to run the tap step, or enrolment, unless a possession factor has already succeeded in the same login. This stops a stranger who knows a username from using the service's lockout to keep the user out ([service.md](service.md), "Lockout as denial of service"), or from enrolling taps on someone else's account.
- One command brings up Keycloak, the service and a demo realm on a laptop.

## Non-goals for this step

- Production packaging: a KMS pepper, mutual TLS between Keycloak and the service, high availability. The service does not have these yet either.
- Deleting the service record when a Keycloak user is deleted. Removing the credential deletes it; a deleted user leaves a record keyed by an opaque id. Planned with the production work.
- A curated image library. The demo ships one generated image.
- A browser-driven end-to-end test. Planned as the next step; see "Testing".

## Delivery

Two pull requests, in order:

1. **Service: staged enrolment and the tap limit.** The changes under "Service changes" below. Small, Rust only, and useful to every adapter, including the OIDC host in spec section 17 step 4.
2. **The plugin and the demo**, on top of it.

## Service changes

The service replaces a record as soon as `/v1/enrol` succeeds, so an adapter cannot let the user confirm a new sequence before it replaces the old one. Building that in the plugin would make an unwritten id convention part of the API that the next adapter must copy. The service gains it instead.

| Method and path | Body | Answer |
|---|---|---|
| `POST /v1/challenge` | unchanged | `200 {"image_id", "max_taps"}`. `max_taps` is the policy's limit, the same for every user, so the tap page can pad to it. |
| `POST /v1/enrolments` | `{"user_id", "image_id", "taps"}` | `201 {"warnings"}`, or `422 {"error"}`. Stores a pending enrolment for the user, replacing any earlier one. The live record and any lockout are untouched. |
| `POST /v1/enrolments/confirm` | `{"user_id", "taps"}` | `200 {"confirmed"}`. Verifies the taps against the pending enrolment. On a match the pending record replaces the live one and clears any lockout. Match or not, the pending enrolment is gone afterwards. |

Rules:

- **One confirmation attempt.** A wrong confirmation discards the pending enrolment, and the user starts again from the first entry. That leaves no record that can be guessed at repeatedly through this endpoint, so confirmation needs no lockout of its own.
- **Pending enrolments expire** after 10 minutes, and are pruned like expired lockout rows.
- **The record comes from the first entry.** Confirmation only proves the user can repeat it. Two Argon2id runs per enrolment.
- **The same cost whether or not anything is pending.** Confirming with nothing pending verifies against a decoy, so timing does not show whether an enrolment is pending.
- **A deletion during confirmation wins.** The record is made live only if its pending enrolment is still the current one once the taps have been checked.
- `DELETE /v1/users/{user_id}` also removes a pending enrolment.
- `/v1/enrol` stays as it is, for tests and direct provisioning.

## Versions

| Component | Choice | Why |
|---|---|---|
| Keycloak | 26.8.2, the latest release | The SPIs used here are stable across 26.x. |
| Java | 21 | The JDK Keycloak 26 runs on. |
| Build | Maven, run inside a `maven` container pinned by digest | No JDK is needed on the host, matching how the service image is built. |
| Runtime dependencies | None beyond what Keycloak provides | `java.net.http` for HTTP, Keycloak's own Jackson for JSON. Nothing extra to vet or licence. |

## Components

All in one provider jar, `keycloak/` at the repository root.

| Piece | Keycloak SPI | Job |
|---|---|---|
| `PixelFactorAuthenticator` and its factory | `Authenticator` | The tap step at login. |
| `PixelFactorEnrolAction` and its factory | `RequiredActionProvider` | Enrolment and re-enrolment. |
| `PixelFactorCredentialProvider` and its factory | `CredentialProvider` | A marker credential, so Keycloak knows who is enrolled and the account console can show and remove it. |
| `PossessionGate` | none | The check shared by the tap step and enrolment. |
| `PixelFactorService` and `HttpServiceClient` | none | The calls to the service. |
| `pixel-factor-tap.ftl` and `tap.js` | theme resources in the jar | The tap page, for login and enrolment. |

### The marker credential

The credential stores no secret: its secret data is empty and its credential data holds only the image id. The record lives in the service. The marker exists so that `configuredFor` can answer "is this user enrolled?" without a network call, so the credential shows in the account console, and so removing it (by the user or an admin) deletes the record at the service.

Re-enrolment updates the existing marker in place. It is never deleted and recreated, because deleting it deletes the record that was just written.

Deleting the marker calls `DELETE /v1/users/{id}`. If the service is unreachable the deletion fails and the marker stays, so Keycloak never shows a user as unenrolled while their record still exists.

The marker and the record can still drift, for example if the service's database is reset while Keycloak's survives. The user then never matches. Recovery is the same as for a forgotten sequence: an admin removes the credential and the user enrols again.

### User ids

The plugin sends Keycloak's internal user id (a UUID), never the username. A rename does not orphan the record, and the service's logs hold no username. UUIDs are unique across realms, so one service can serve several realms.

## Configuration

Server-wide, through Keycloak's SPI options, so the API token never sits in the realm database or the admin console. The authenticator's factory reads them; the other factories read the same scope through `Config.scope("authenticator", "pixel-factor")`, so each value is set once.

| Option | Environment variable | Meaning |
|---|---|---|
| `spi-authenticator--pixel-factor--service-url` | `KC_SPI_AUTHENTICATOR__PIXEL_FACTOR__SERVICE_URL` | Base URL of the service. |
| `spi-authenticator--pixel-factor--api-token` | `KC_SPI_AUTHENTICATOR__PIXEL_FACTOR__API_TOKEN` | The service's `PF_API_TOKEN`. |

The option form is from Keycloak's provider configuration guide; the environment form follows its naming rule and is confirmed by the demo smoke test. A missing URL or token fails Keycloak at start, so a misconfigured server does not come up half working.

Fixed in code, deliberately not settings:

- **The possession providers:** `webauthn-authenticator`, `webauthn-authenticator-passwordless` and `auth-otp-form`. A setting would let an operator add a provider that is not a possession factor and quietly defeat the gate. Adding one is a code change and a review.
- **A 5-second timeout** on each service call.

## The possession gate

Passes when the login's execution statuses include an execution whose provider id is one of the possession providers and whose status is `SUCCESS`. Anything else fails it, including a possession execution that is only `SETUP_REQUIRED`.

On failure the flow ends with `ACCESS_DENIED`, an error is logged naming the misconfiguration, and the service is never called. It fails closed: a flow edited to put the tap step first stops working rather than becoming abusable.

So a user must already hold a possession factor before they can enrol taps or use them. It is set up out of band: by an admin (for example Keycloak's "execute actions" email), or in the demo by the realm import. A user whose possession factor is removed cannot sign in until an admin sets one up again.

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

1. The possession gate.
2. `POST /v1/challenge` with the user id. Keep `image_id` and `max_taps` in an authentication session note (neither is secret), then render the tap page.

`action` (the form post):

1. Parse the taps (see "The tap page"). Anything unparseable becomes an empty sequence, which the service answers with no match. The plugin never rejects taps on its own, so it adds no signal the service does not give.
2. `POST /v1/verify`.
3. `match: true`: `context.success()`.
4. `match: false, locked: false`: `context.failureChallenge(AuthenticationFlowError.INVALID_CREDENTIALS, page)`, which records the failure for Keycloak's brute-force detection, with "That sequence did not match."
5. `locked: true`: the same call, with "Too many attempts. Try again later." Telling the user is safe: they have already passed the possession factor, and the service sets the flag for unknown ids too.

`configuredFor` is true when the user has the marker credential. A user without one gets the enrolment required action and must finish it before the login completes.

## Enrolment

Run as a required action, either for a user who has never enrolled (set by `configuredFor`) or on request: the user from the account console, or an admin. It starts with the possession gate, so it cannot run on a username alone. Re-enrolment from the account console is an application-initiated action: the action sets a maximum authentication age of zero, so Keycloak makes the user log in afresh and the possession factor succeeds in the same session the gate checks. A session resumed from a cookie has no possession `SUCCESS`, so it does not pass; the demo smoke test checks this.

1. The possession gate.
2. `POST /v1/challenge`, as at login. Show the image.
3. First entry: `POST /v1/enrolments`. A `422` for the sequence shows its reason and asks again. Any warnings ("taps 2 and 3 are close together") are shown, with a choice to continue or start again.
4. Confirmation entry: `POST /v1/enrolments/confirm`. `confirmed: false`: "That did not match your first entry", back to step 3.
5. `confirmed: true`: create the marker, or update it in place with the new image id. The required action completes.

The taps are never held in Keycloak between requests. A user who abandons enrolment keeps their old record and marker; the pending enrolment expires.

## Service errors

For every call, a transport failure, a timeout, or any answer other than the ones listed above ends the flow with `INTERNAL_ERROR` and a generic page, and logs the status. That includes `401` (a wrong token) and every `422` except the enrolment one in step 3. Unknown image is a `422` too, but a deployment error, not something the user can fix, so it is treated as an error. Nothing falls through to success, and a failed enrolment leaves the required action pending.

## The tap page

Follows spec section 12.

- The whole image, letterboxed, never cropped. Pinch zoom is allowed on phones.
- Each tap is converted to units of image width: x and y divided by the rendered width, so y runs 0 to the aspect. Taps on the letterbox bars, outside the image, are ignored on the page.
- The taps of the current entry are marked with numbered dots, with Undo and Clear. These are the user's own input on their own screen, like a password field's dots; nothing about earlier entries or the enrolled points is ever shown (non-goal "No hints"). The page stops accepting taps at `max_taps`.
- **Fixed-length submission.** The form field always holds `max_taps` coordinate pairs, each formatted to a fixed width, with unused pairs as a fixed filler the plugin strips. `max_taps` comes from the service and is the same for every user, so the request size is the same for every sequence and a network observer cannot count taps from it.
- The page sends taps and nothing else; it never receives offsets, records or the pepper.
- **Keyboard and assistive technology.** The image takes keyboard focus: arrow keys move a cursor (Shift for bigger steps), Enter or Space places a tap there and Backspace removes the last one, so keyboard and switch users can enter a sequence. A polite live region announces only "Point added", "Point removed" and "All points cleared", never a position. The factor is visual by nature, so a user who cannot see the image cannot use it; such a user needs a realm flow with another factor in its place, which is the operator's decision.

The script is plain JavaScript with no external libraries, served from the jar, under Keycloak's default content security policy.

## Images

Keycloak and the service must agree on image ids and aspects. The jar carries image files at `resources/img/<image id>.<ext>` in the theme resources; the service's `PF_IMAGES` lists the same ids. A challenge for an id the jar does not have fails with `INTERNAL_ERROR` and is logged.

The aspect is not checked: an image file whose height over width differs from its `PF_IMAGES` entry would shift every tap and break matching. Keeping them equal is an operator duty, stated in [OPERATIONS.md](OPERATIONS.md) when this lands. The demo smoke test checks it for the demo image.

The demo ships one image, `demo`, aspect 0.75, drawn by a script in the style of `docs/images/make_diagrams.py`: an abstract scene with more than 30 distinct features, no faces and no text (spec section 11). Real curated images, and their licensing, are a later decision.

## Demo

A `docker compose` file in `keycloak/demo/` runs:

- `keycloak`: the official Keycloak image pinned by digest, in development mode, with the plugin jar mounted and a realm imported from `realm.json`.
- `service`: the image from the repository `Dockerfile`, with development settings and a fresh volume.

The realm has the flow above with OTP as the possession factor, and one user whose OTP credential is part of the import. The demo prints its secret, to add to an authenticator app. The user enrols taps on first login, after the one-time code. Only Keycloak's port is published; the service is reachable only from Keycloak on the compose network, as [OPERATIONS.md](OPERATIONS.md) expects.

## Commands

New Makefile targets, all running Maven inside the pinned container:

| Target | Does |
|---|---|
| `plugin-check` | Format check, compile with warnings as errors, unit tests. |
| `plugin-fmt` | Format the plugin's Java code. |
| `plugin-jar` | Build the jar into `keycloak/target`. |
| `demo` | Build the jar and the service image, start the compose stack, print the login URL and the OTP secret. |
| `demo-smoke` | Start the stack on a free port; check that Keycloak loaded the providers and imported the realm; from inside the network, drive the service calls the plugin uses (challenge, staged enrolment, confirmation, verify, delete); check the demo image's aspect; then log in through Keycloak's own pages with a generated one-time code: enrol, confirm, fail with a wrong sequence, succeed with the right one, and check that re-enrolment asks for a fresh login; stop it. |

`make check` stays Rust-only and offline. Maven needs the network the first time it fetches Keycloak's jars, which would break that promise; the plugin targets are separate, and CI runs `plugin-check` and `demo-smoke` on every pull request. Dependabot gains the `maven` and `docker-compose` ecosystems.

## Testing

- **Service** (pull request 1): the staged enrolment rules above, each as a test in `service/tests/api.rs`, and the new calls added to `scripts/smoke.sh`.
- **Plugin unit tests** (JUnit 5, Mockito for Keycloak's context objects): the possession gate passes only on a listed provider with `SUCCESS` and fails closed otherwise, including `SETUP_REQUIRED`; every service outcome, including each error in "Service errors", maps to the right flow result; nothing yields success on an error; the tap parser turns any bad input into an empty sequence; enrolment calls the service in order and writes the marker only after a confirmation; re-enrolment updates the marker rather than replacing it.
- **Demo smoke**, as above. It exercises `HttpServiceClient`'s calls against the real service image, and the plugin inside Keycloak through `keycloak/demo/login_smoke.py`, which posts the taps field as the page's script builds it.
- **Next step, not in this one**: a Playwright test that taps the image in a real browser, so `pixel-factor-tap.js` itself is exercised.

## Assumptions

Recorded rather than asked; each can be revisited.

1. Maven over Gradle, built in a container rather than on the host.
2. Plugin targets are kept out of `make check` to keep it offline.
3. One confirmation attempt per pending enrolment; a wrong one starts enrolment again.
4. Re-enrolment needs a full login or an admin, not just the possession factor.
5. The demo uses OTP rather than a passkey, because it can be scripted later.
6. One generated demo image; curated photographs come later.
7. `/v1/enrol` is kept alongside staged enrolment.

## Docs to update when this lands

- `AGENTS.md` and `README.md`: the plugin moves from "planned" to the layout and commands tables.
- `PRODUCT.md`: the "Easy to adopt" row, which still says the service does not exist.
- `spec.md` section 14: reset needs a full login or an admin, not the possession factor alone. Section 17: step 3 status.
- `service.md`: the new endpoints and rules, and the lockout paragraph's "the Keycloak plugin will enforce that order".
- `OPERATIONS.md`: image aspects must match `PF_IMAGES`.
- `HOW-IT-WORKS.md`: the plugin is no longer "planned, not built yet".
