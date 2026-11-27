//! HTTP service around `pixel-factor-core`: enrolment, verification and
//! record storage. See `docs/service.md` for the API and the decisions behind it.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::todo,
    clippy::unimplemented,
    clippy::dbg_macro
)]

pub mod config;
pub mod store;

use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{middleware, Json, Router};
use hmac::{Hmac, KeyInit, Mac};
use pixel_factor_core::geometry::Point;
use pixel_factor_core::{
    enrol, verify, Error as CoreError, KdfParams, PepperProvider, Policy, Record, StaticPepper,
    Warning,
};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use store::{Store, StoreError};
use subtle::ConstantTimeEq;
use tokio::sync::Semaphore;

/// Largest request body accepted, in bytes.
pub const MAX_BODY: usize = 16 * 1024;

/// Longest user id accepted, in bytes.
pub const MAX_USER_ID_LEN: usize = 255;

const DOMAIN_CHALLENGE: &[u8] = b"pfsvc/challenge";

/// The commit the binary was built from, when the build set `PF_BUILD_COMMIT`.
pub const BUILD_COMMIT: &str = match option_env!("PF_BUILD_COMMIT") {
    Some(c) => c,
    None => "unknown",
};

/// When repeated failures lock a user, and for how long.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lockout {
    /// Consecutive failures that start a lock.
    pub max_failures: u32,
    /// Length of the first lock, in seconds. Each further failure doubles it.
    pub base_secs: u64,
    /// Longest lock, in seconds.
    pub max_secs: u64,
    /// Failures older than this many seconds are forgotten. At least
    /// `max_secs`, so a lock always ends before its failures are dropped.
    pub window_secs: u64,
}

impl Lockout {
    /// Unix time until which a user with `failures` consecutive failures is
    /// locked, or 0 when they are not locked.
    pub fn until(&self, failures: u32, now: u64) -> u64 {
        let Some(extra) = failures.checked_sub(self.max_failures) else {
            return 0;
        };
        let secs = 2u64
            .checked_pow(extra)
            .and_then(|m| self.base_secs.checked_mul(m))
            .unwrap_or(u64::MAX)
            .min(self.max_secs);
        now.saturating_add(secs)
    }
}

/// Shared state behind every request.
pub struct App {
    store: Mutex<Store>,
    pepper: Box<dyn PepperProvider + Send + Sync>,
    images: BTreeMap<String, f64>,
    decoys: BTreeMap<String, Record>,
    token_key: [u8; 32],
    token_mac: [u8; 32],
    lockout: Lockout,
    kdf: KdfParams,
    work: Semaphore,
}

impl App {
    /// Build the state. `kdf` is the Argon2id cost for new records and
    /// decoys: `KdfParams::default()` in production, cheaper in tests.
    pub fn new(config: &config::Config, store: Store, kdf: KdfParams) -> Result<Self, String> {
        let pepper = StaticPepper::new(1, config.dev_pepper.clone());
        let mut token_key = [0u8; 32];
        getrandom::fill(&mut token_key).map_err(|e| format!("random key: {e}"))?;
        let token_mac = token_mac(&token_key, &config.api_token).ok_or("token key rejected")?;
        let mut decoys = BTreeMap::new();
        for (id, aspect) in &config.images {
            let record = enrol(&decoy_taps(*aspect), id, &policy(*aspect, kdf), &pepper)
                .map_err(|e| format!("decoy record for image '{id}': {e}"))?
                .record;
            decoys.insert(id.clone(), record);
        }
        Ok(Self {
            store: Mutex::new(store),
            pepper: Box::new(pepper),
            images: config.images.clone(),
            decoys,
            token_key,
            token_mac,
            lockout: config.lockout,
            kdf,
            work: Semaphore::new(config.max_concurrent),
        })
    }

    /// The image an unknown user is shown: stable for the id, unpredictable
    /// without the pepper.
    fn decoy_image(&self, user_id: &str) -> Result<&str, ApiError> {
        let mut msg = DOMAIN_CHALLENGE.to_vec();
        msg.push(0);
        msg.extend_from_slice(user_id.as_bytes());
        let mac = self.pepper.hmac(self.pepper.current_version(), &msg)?;
        let mut head = [0u8; 8];
        head.copy_from_slice(mac.get(..8).unwrap_or(&[0; 8]));
        let count = u64::try_from(self.images.len()).unwrap_or(1).max(1);
        let pick = usize::try_from(u64::from_be_bytes(head) % count).unwrap_or(0);
        self.images
            .keys()
            .nth(pick)
            .map(String::as_str)
            .ok_or_else(|| ApiError::Internal("no images configured".to_owned()))
    }
}

fn policy(aspect: f64, kdf: KdfParams) -> Policy {
    Policy {
        kdf,
        ..Policy::standard(aspect)
    }
}

/// A valid sequence for any aspect: five points along the middle of the image.
/// Its result is always discarded, so it does not matter that it is public.
fn decoy_taps(aspect: f64) -> Vec<Point> {
    [0.1, 0.3, 0.5, 0.7, 0.9]
        .into_iter()
        .map(|x| Point::new(x, aspect / 2.0))
        .collect()
}

fn token_mac(key: &[u8; 32], token: &str) -> Option<[u8; 32]> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).ok()?;
    mac.update(token.as_bytes());
    Some(mac.finalize().into_bytes().into())
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The router, with every route and limit in place.
pub fn router(app: Arc<App>) -> Router {
    let v1 = Router::new()
        .route("/v1/challenge", post(challenge))
        .route("/v1/enrol", post(enrol_user))
        .route("/v1/verify", post(verify_user))
        .route("/v1/users/{user_id}", delete(delete_user))
        .route_layer(middleware::from_fn_with_state(app.clone(), require_token));
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/version", get(version))
        .merge(v1)
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .with_state(app)
}

async fn require_token(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: middleware::Next,
) -> Response {
    let presented = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    // MACs under a per-process random key are compared, so the comparison
    // takes the same time whatever the length of the token presented.
    let presented = token_mac(&app.token_key, presented).unwrap_or([0; 32]);
    if bool::from(presented.ct_eq(&app.token_mac)) {
        next.run(request).await
    } else {
        ApiError::Unauthorised.into_response()
    }
}

#[derive(Serialize)]
struct VersionAnswer {
    version: &'static str,
    commit: &'static str,
}

async fn version() -> Json<VersionAnswer> {
    Json(VersionAnswer {
        version: env!("CARGO_PKG_VERSION"),
        commit: BUILD_COMMIT,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChallengeRequest {
    user_id: String,
}

#[derive(Serialize)]
struct ChallengeAnswer {
    image_id: String,
}

async fn challenge(
    State(app): State<Arc<App>>,
    Json(req): Json<ChallengeRequest>,
) -> Result<Json<ChallengeAnswer>, ApiError> {
    check_user_id(&req.user_id)?;
    let app2 = app.clone();
    let user_id = req.user_id;
    let image_id = blocking(move || {
        let stored = lock(&app2.store)?.get(&user_id)?;
        match stored {
            Some(record) => Ok(record.image_id),
            None => app2.decoy_image(&user_id).map(str::to_owned),
        }
    })
    .await?;
    Ok(Json(ChallengeAnswer { image_id }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnrolRequest {
    user_id: String,
    image_id: String,
    taps: Vec<[f64; 2]>,
}

#[derive(Serialize)]
struct EnrolAnswer {
    warnings: Vec<String>,
}

async fn enrol_user(
    State(app): State<Arc<App>>,
    Json(req): Json<EnrolRequest>,
) -> Result<(StatusCode, Json<EnrolAnswer>), ApiError> {
    check_user_id(&req.user_id)?;
    let Some(aspect) = app.images.get(&req.image_id).copied() else {
        return Err(ApiError::Rejected("unknown image".to_owned()));
    };
    let _permit = app
        .work
        .acquire()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let app2 = app.clone();
    let warnings = blocking(move || {
        let taps = points(&req.taps);
        let enrolment = enrol(
            &taps,
            &req.image_id,
            &policy(aspect, app2.kdf),
            app2.pepper.as_ref(),
        )?;
        lock(&app2.store)?.put(&req.user_id, &enrolment.record)?;
        tracing::info!(user_id = ?req.user_id, image_id = ?req.image_id, "enrolled");
        Ok(enrolment.warnings.iter().map(describe).collect())
    })
    .await?;
    Ok((StatusCode::CREATED, Json(EnrolAnswer { warnings })))
}

fn describe(warning: &Warning) -> String {
    match warning {
        Warning::PointsClose { first, second } => {
            format!(
                "taps {} and {} are close together",
                first.saturating_add(1),
                second.saturating_add(1)
            )
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyRequest {
    user_id: String,
    taps: Vec<[f64; 2]>,
}

/// The answer to a verification.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyAnswer {
    /// The taps matched the enrolled sequence.
    #[serde(rename = "match")]
    pub matched: bool,
    /// The user is locked after repeated failures.
    pub locked: bool,
}

async fn verify_user(
    State(app): State<Arc<App>>,
    Json(req): Json<VerifyRequest>,
) -> Result<Json<VerifyAnswer>, ApiError> {
    check_user_id(&req.user_id)?;
    let _permit = app
        .work
        .acquire()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let app2 = app.clone();
    let answer = blocking(move || verify_blocking(&app2, &req.user_id, &points(&req.taps))).await?;
    Ok(Json(answer))
}

/// Every path runs one full verification, against the user's record or a
/// decoy, and the same failure bookkeeping, so neither the time taken nor the
/// answer shows whether the user exists. Unknown ids are counted and locked
/// exactly like known ones.
///
/// An attempt is counted as a failure before the taps are checked and cleared
/// only on a match, so concurrent attempts cannot all slip in under the limit.
fn verify_blocking(app: &App, user_id: &str, taps: &[Point]) -> Result<VerifyAnswer, ApiError> {
    let now = unix_now();
    let window = app.lockout.window_secs;
    let (record, attempt) = {
        let store = lock(&app.store)?;
        let record = store.get(user_id)?;
        let failures = store.failures(user_id, now, window)?;
        let attempt = if now < failures.locked_until {
            None
        } else {
            Some(store.add_failure(user_id, now, window)?)
        };
        (record, attempt)
    };
    let matched = match &record {
        Some(record) => verify(taps, record, app.pepper.as_ref())?,
        None => {
            let image = app.decoy_image(user_id)?;
            let decoy = app
                .decoys
                .get(image)
                .ok_or_else(|| ApiError::Internal("missing decoy".to_owned()))?;
            // The decoy's taps are public, so its result must never count.
            let _ = verify(taps, decoy, app.pepper.as_ref())?;
            false
        }
    };
    let locked_answer = VerifyAnswer {
        matched: false,
        locked: true,
    };
    let Some(count) = attempt else {
        tracing::info!(user_id = ?user_id, outcome = "locked", "verified");
        return Ok(locked_answer);
    };
    let store = lock(&app.store)?;
    // A count over the limit means more attempts were in flight at once than
    // the limit allows: this one does not get to match.
    if matched && count <= app.lockout.max_failures {
        store.clear_failures(user_id)?;
        tracing::info!(user_id = ?user_id, outcome = "match", "verified");
        return Ok(VerifyAnswer {
            matched: true,
            locked: false,
        });
    }
    let locked_until = app.lockout.until(count, now);
    store.set_locked_until(user_id, locked_until)?;
    store.prune_failures(now, window)?;
    let locked = locked_until > now;
    tracing::info!(user_id = ?user_id, outcome = "no_match", failures = count, locked, "verified");
    if locked {
        return Ok(locked_answer);
    }
    Ok(VerifyAnswer {
        matched: false,
        locked: false,
    })
}

async fn delete_user(
    State(app): State<Arc<App>>,
    Path(user_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    check_user_id(&user_id)?;
    let app2 = app.clone();
    blocking(move || {
        lock(&app2.store)?.delete(&user_id)?;
        tracing::info!(user_id = ?user_id, "deleted");
        Ok(())
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn check_user_id(user_id: &str) -> Result<(), ApiError> {
    // Control characters are refused so that a user id cannot forge log lines.
    if user_id.is_empty()
        || user_id.len() > MAX_USER_ID_LEN
        || user_id.chars().any(char::is_control)
    {
        return Err(ApiError::Rejected(format!(
            "user_id must be 1 to {MAX_USER_ID_LEN} bytes with no control characters"
        )));
    }
    Ok(())
}

fn points(taps: &[[f64; 2]]) -> Vec<Point> {
    taps.iter().map(|[x, y]| Point::new(*x, *y)).collect()
}

fn lock(store: &Mutex<Store>) -> Result<std::sync::MutexGuard<'_, Store>, ApiError> {
    store
        .lock()
        .map_err(|_| ApiError::Internal("store lock poisoned".to_owned()))
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
}

/// Why a request failed.
#[derive(Debug)]
pub enum ApiError {
    /// Missing or wrong bearer token.
    Unauthorised,
    /// The request is well formed but not acceptable. The message is safe to
    /// return: it describes the request, never stored state.
    Rejected(String),
    /// Anything else. Logged, and returned only as a generic message.
    Internal(String),
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        Self::Internal(e.to_string())
    }
}

impl From<CoreError> for ApiError {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::InvalidRecord(_)
            | CoreError::Kdf(_)
            | CoreError::Pepper(_)
            | CoreError::InvalidPolicy(_) => Self::Internal(e.to_string()),
            other => Self::Rejected(rejection(&other)),
        }
    }
}

fn rejection(e: &CoreError) -> String {
    match e {
        CoreError::PointOutOfBounds { tap } => {
            format!("tap {} is outside the image", tap.saturating_add(1))
        }
        CoreError::TapCount { min, max, got } => {
            format!("{got} taps given; between {min} and {max} are needed")
        }
        CoreError::ConsecutiveTooClose { tap } => {
            format!(
                "taps {} and {} are too close together",
                tap,
                tap.saturating_add(1)
            )
        }
        CoreError::TooFewDistinct { required, got } => {
            format!("{got} distinct points given; at least {required} are needed")
        }
        other => other.to_string(),
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error) = match self {
            Self::Unauthorised => (
                StatusCode::UNAUTHORIZED,
                "missing or wrong bearer token".to_owned(),
            ),
            Self::Rejected(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
            Self::Internal(msg) => {
                tracing::error!(error = %msg, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal error".to_owned(),
                )
            }
        };
        (status, Json(ErrorBody { error })).into_response()
    }
}
