//! Breach-resistant verification of an ordered sequence of taps on an image.
//!
//! A user enrols by tapping a sequence of points on an image. Later taps are
//! accepted if each lands within a hexagonal tolerance of the enrolled point,
//! in the same order. The stored [`Record`] cannot be used to recover or guess
//! the sequence without a pepper that is held outside the database.
//!
//! How it works:
//!
//! * Each point is split into a secret cell index and a public offset
//!   ([`geometry::Lattice::split`]). Subtracting the offset from any nearby
//!   tap and rounding to the nearest hexagon centre recovers the index.
//! * The first offset is stored in the clear. Each later offset is encrypted
//!   under a key derived from the pepper and all earlier indices, so a stolen
//!   record does not show which taps revisit a point.
//! * One verifier covers the whole ordered sequence:
//!   `HMAC(pepper, Argon2id(indices, salt))`. There is never a hash per point.
//!
//! This crate does no I/O. Rate limiting, lockout, and record storage belong
//! to the caller.

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

pub mod geometry;

use argon2::{Algorithm, Argon2, Params, Version};
use geometry::{CellIndex, Lattice, Offset, Point, MAX_ASPECT};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

/// Format version written into every record.
pub const RECORD_VERSION: u8 = 2;

/// Most taps a sequence may have. The index count is stored in one byte.
pub const MAX_TAPS: usize = 32;

/// Length of [`Record::enc_offsets`] in every record: one 4-byte slot for each
/// tap after the first, up to [`MAX_TAPS`]. Slots past the end of the sequence
/// hold random fill, so the record does not reveal how many taps it has.
pub const ENC_OFFSETS_LEN: usize = (MAX_TAPS - 1) * 4;

/// Longest image id accepted, in bytes.
pub const MAX_IMAGE_ID_LEN: usize = 255;

const DOMAIN_OFFSET: &[u8] = b"pfv1/offset";
const DOMAIN_KDF: &[u8] = b"pfv1/kdf";
const DOMAIN_FINAL: &[u8] = b"pfv1/final";

/// Errors from enrolment or from the pepper provider.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// Policy values are out of range.
    InvalidPolicy(&'static str),
    /// The image id is empty or longer than [`MAX_IMAGE_ID_LEN`].
    InvalidImageId,
    /// A tap is not a finite coordinate inside the image.
    PointOutOfBounds {
        /// Zero-based position of the tap in the sequence.
        tap: usize,
    },
    /// Wrong number of taps for the policy.
    TapCount {
        /// Fewest taps the policy allows.
        min: usize,
        /// Most taps the policy allows.
        max: usize,
        /// Number of taps supplied.
        got: usize,
    },
    /// Two consecutive taps are less than two circumradii apart.
    ConsecutiveTooClose {
        /// Zero-based position of the second tap of the pair.
        tap: usize,
    },
    /// Not enough distinct points in the sequence.
    TooFewDistinct {
        /// Distinct points the policy requires.
        required: usize,
        /// Distinct points supplied.
        got: usize,
    },
    /// The record is malformed.
    InvalidRecord(&'static str),
    /// Argon2 rejected its parameters or failed.
    Kdf(String),
    /// The pepper provider failed, or does not hold the requested version.
    Pepper(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Error {}

/// Performs the keyed HMAC. In production this is a KMS or HSM call, so the
/// pepper never sits in application memory or beside the database.
pub trait PepperProvider {
    /// The version new records should be written with.
    fn current_version(&self) -> u32;
    /// HMAC-SHA256 of `msg` under the pepper with the given version.
    fn hmac(&self, version: u32, msg: &[u8]) -> Result<[u8; 32], Error>;
}

/// An in-memory pepper. For development and tests only.
pub struct StaticPepper {
    version: u32,
    key: Vec<u8>,
}

impl StaticPepper {
    /// A pepper with the given version number and key bytes.
    pub fn new(version: u32, key: impl Into<Vec<u8>>) -> Self {
        Self {
            version,
            key: key.into(),
        }
    }
}

impl PepperProvider for StaticPepper {
    fn current_version(&self) -> u32 {
        self.version
    }

    fn hmac(&self, version: u32, msg: &[u8]) -> Result<[u8; 32], Error> {
        if version != self.version {
            return Err(Error::Pepper(format!("unknown pepper version {version}")));
        }
        let mut mac =
            HmacSha256::new_from_slice(&self.key).map_err(|e| Error::Pepper(e.to_string()))?;
        mac.update(msg);
        Ok(mac.finalize().into_bytes().into())
    }
}

/// Argon2id cost parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Memory in KiB.
    pub m_kib: u32,
    /// Iterations.
    pub t: u32,
    /// Parallelism.
    pub p: u32,
}

impl Default for KdfParams {
    /// 64 MiB, 3 iterations, parallelism 1.
    fn default() -> Self {
        Self {
            m_kib: 64 * 1024,
            t: 3,
            p: 1,
        }
    }
}

/// Enrolment rules.
#[derive(Clone, Debug, PartialEq)]
pub struct Policy {
    /// Guaranteed tolerance, as a fraction of image width.
    pub r: f64,
    /// Image height divided by width.
    pub aspect: f64,
    /// Minimum number of distinct points.
    pub min_distinct: usize,
    /// Most taps in a sequence, revisits included. At most [`MAX_TAPS`].
    pub max_taps: usize,
    /// Argon2id cost for the verifier.
    pub kdf: KdfParams,
}

impl Policy {
    /// r = 2.2% of width, 5 to 8 taps with at least 5 distinct points, default
    /// Argon2id. Revisits are unlimited within the tap count.
    pub fn standard(aspect: f64) -> Self {
        Self {
            r: 0.022,
            aspect,
            min_distinct: 5,
            max_taps: 8,
            kdf: KdfParams::default(),
        }
    }

    fn validate(&self) -> Result<(), Error> {
        if Lattice::new(self.r).is_none() {
            return Err(Error::InvalidPolicy("r must be in [0.0001, 0.25)"));
        }
        if !valid_aspect(self.aspect) {
            return Err(Error::InvalidPolicy("aspect must be in (0, 16]"));
        }
        if self.min_distinct < 2 {
            return Err(Error::InvalidPolicy("min_distinct must be at least 2"));
        }
        if self.max_taps < self.min_distinct {
            return Err(Error::InvalidPolicy(
                "max_taps must be at least min_distinct",
            ));
        }
        if self.max_taps > MAX_TAPS {
            return Err(Error::InvalidPolicy("at most 32 taps"));
        }
        Ok(())
    }
}

/// What gets stored for a user. Safe to keep in the database: without the
/// pepper it cannot be checked against guesses.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// Record format version. See [`RECORD_VERSION`].
    pub version: u8,
    /// Which image the sequence was enrolled on.
    pub image_id: String,
    /// Guaranteed tolerance, as a fraction of image width.
    pub r: f64,
    /// Image height divided by width.
    pub aspect: f64,
    /// Argon2id cost used for the verifier.
    pub kdf: KdfParams,
    /// Which pepper the record was written with.
    pub pepper_version: u32,
    /// Random per-record salt.
    #[serde(with = "hex")]
    pub salt: [u8; 16],
    /// Offset of the first tap, in the clear.
    #[serde(with = "hex")]
    pub first_offset: [u8; 4],
    /// Encrypted offsets of taps 2..n, four bytes each, then random fill to
    /// [`ENC_OFFSETS_LEN`] bytes. Without the pepper the fill cannot be told
    /// apart from an encrypted offset, so the tap count stays secret.
    #[serde(with = "hex")]
    pub enc_offsets: Vec<u8>,
    /// The one hash covering the whole ordered sequence.
    #[serde(with = "hex")]
    pub verifier: [u8; 32],
}

impl Record {
    /// Bytes that bind the keys and verifier to this record's parameters, so
    /// a record cannot be reused under a different image or tolerance.
    fn context(&self) -> Vec<u8> {
        context(&self.image_id, self.r, self.aspect)
    }
}

/// Non-fatal findings from enrolment.
#[derive(Clone, Debug, PartialEq)]
pub enum Warning {
    /// Two distinct points are closer than 4r. Valid, but fiddly on a phone.
    PointsClose {
        /// Zero-based position of the earlier tap.
        first: usize,
        /// Zero-based position of the later tap.
        second: usize,
    },
}

/// Result of a successful enrolment.
#[derive(Clone, Debug)]
pub struct Enrolment {
    /// What to store for the user.
    pub record: Record,
    /// Things worth telling the user. Enrolment still succeeded.
    pub warnings: Vec<Warning>,
}

/// Enrol a sequence of taps.
pub fn enrol(
    taps: &[Point],
    image_id: &str,
    policy: &Policy,
    pepper: &dyn PepperProvider,
) -> Result<Enrolment, Error> {
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|e| Error::Kdf(e.to_string()))?;
    let mut fill = [0u8; ENC_OFFSETS_LEN];
    getrandom::fill(&mut fill).map_err(|e| Error::Kdf(e.to_string()))?;
    enrol_deterministic(taps, image_id, policy, pepper, salt, fill)
}

/// Enrol with caller-supplied randomness: the salt, and the fill for the
/// unused offset slots. Exists so that test vectors are reproducible.
/// Production code should call [`enrol`]: fill that is not random reveals the
/// tap count.
pub fn enrol_deterministic(
    taps: &[Point],
    image_id: &str,
    policy: &Policy,
    pepper: &dyn PepperProvider,
    salt: [u8; 16],
    fill: [u8; ENC_OFFSETS_LEN],
) -> Result<Enrolment, Error> {
    policy.validate()?;
    if !valid_image_id(image_id) {
        return Err(Error::InvalidImageId);
    }
    let lattice = Lattice::new(policy.r).ok_or(Error::InvalidPolicy("r out of range"))?;

    let (min, max) = (policy.min_distinct, policy.max_taps);
    if taps.len() < min || taps.len() > max {
        return Err(Error::TapCount {
            min,
            max,
            got: taps.len(),
        });
    }
    if let Some(i) = taps.iter().position(|p| !in_bounds(p, policy.aspect)) {
        return Err(Error::PointOutOfBounds { tap: i });
    }

    // Snap revisits to the earlier point. `fresh` marks the taps that
    // introduced a new point.
    let mut points: Vec<Point> = Vec::with_capacity(taps.len());
    let mut fresh: Vec<bool> = Vec::with_capacity(taps.len());
    for (i, tap) in taps.iter().enumerate() {
        if let Some(previous) = points.last() {
            if tap.distance(previous) < 2.0 * lattice.circumradius() {
                return Err(Error::ConsecutiveTooClose { tap: i });
            }
        }
        let revisited = points
            .iter()
            .zip(&fresh)
            .find(|(p, is_fresh)| **is_fresh && lattice.within_tolerance(**p, *tap))
            .map(|(p, _)| *p);
        match revisited {
            Some(p) => {
                points.push(p);
                fresh.push(false);
            }
            None => {
                points.push(*tap);
                fresh.push(true);
            }
        }
    }

    let distinct: Vec<(usize, Point)> = points
        .iter()
        .copied()
        .enumerate()
        .zip(&fresh)
        .filter(|(_, is_fresh)| **is_fresh)
        .map(|(indexed, _)| indexed)
        .collect();
    if distinct.len() < policy.min_distinct {
        return Err(Error::TooFewDistinct {
            required: policy.min_distinct,
            got: distinct.len(),
        });
    }

    let mut warnings = Vec::new();
    for (n, (i, a)) in distinct.iter().enumerate() {
        for (j, b) in distinct.iter().skip(n.saturating_add(1)) {
            if a.distance(b) < 4.0 * policy.r {
                warnings.push(Warning::PointsClose {
                    first: *i,
                    second: *j,
                });
            }
        }
    }

    let pepper_version = pepper.current_version();
    let ctx = context(image_id, policy.r, policy.aspect);
    let mut indices: Vec<CellIndex> = Vec::with_capacity(points.len());
    let mut first_offset = [0u8; 4];
    let mut enc_offsets = Vec::with_capacity(ENC_OFFSETS_LEN);
    for p in &points {
        let (index, offset) = lattice.split(*p);
        if indices.is_empty() {
            first_offset = offset.to_bytes();
        } else {
            let ks = offset_keystream(pepper, pepper_version, &salt, &ctx, &indices)?;
            enc_offsets.extend_from_slice(&xor4(offset.to_bytes(), ks));
        }
        indices.push(index);
    }
    // Slots past the last tap take the fill, so every record is the same size.
    enc_offsets.extend(fill.iter().skip(enc_offsets.len()));

    let verifier = compute_verifier(pepper, pepper_version, &salt, &ctx, &policy.kdf, &indices)?;

    Ok(Enrolment {
        record: Record {
            version: RECORD_VERSION,
            image_id: image_id.to_owned(),
            r: policy.r,
            aspect: policy.aspect,
            kdf: policy.kdf,
            pepper_version,
            salt,
            first_offset,
            enc_offsets,
            verifier,
        },
        warnings,
    })
}

/// Check a sequence of taps against a record.
///
/// Returns `Ok(true)` only if every tap matches, in order. The result is all
/// or nothing: there is no indication of which tap was wrong. `Err` means the
/// record or the pepper provider is at fault, not the taps.
///
/// The record does not say how many taps it holds, so any count from 1 to
/// [`MAX_TAPS`] is tried in full, at the same cost whatever the enrolled
/// count. A wrong count fails at the verifier like any other wrong sequence.
pub fn verify(taps: &[Point], record: &Record, pepper: &dyn PepperProvider) -> Result<bool, Error> {
    if record.version != RECORD_VERSION {
        return Err(Error::InvalidRecord("unsupported version"));
    }
    if record.enc_offsets.len() != ENC_OFFSETS_LEN {
        return Err(Error::InvalidRecord("enc_offsets length"));
    }
    if !valid_aspect(record.aspect) || !valid_image_id(&record.image_id) {
        return Err(Error::InvalidRecord("geometry or image id"));
    }
    let lattice = Lattice::new(record.r).ok_or(Error::InvalidRecord("r out of range"))?;

    // These depend only on the taps and on public fields, never on the
    // enrolled count, so returning early reveals nothing about it.
    if taps.is_empty() || taps.len() > MAX_TAPS || taps.iter().any(|p| !in_bounds(p, record.aspect))
    {
        return Ok(false);
    }

    let ctx = record.context();
    let mut encrypted = record.enc_offsets.as_chunks::<4>().0.iter();
    let mut indices: Vec<CellIndex> = Vec::with_capacity(taps.len());
    for tap in taps {
        let offset = if indices.is_empty() {
            Offset::from_bytes(record.first_offset)
        } else {
            let ks = offset_keystream(pepper, record.pepper_version, &record.salt, &ctx, &indices)?;
            let enc: [u8; 4] = *encrypted
                .next()
                .ok_or(Error::InvalidRecord("enc_offsets length"))?;
            // No integrity check on purpose: a wrong key must give a
            // plausible offset, never an error, or taps become checkable
            // one at a time.
            Offset::from_bytes(xor4(enc, ks))
        };
        indices.push(lattice.locate(*tap, offset));
    }

    let candidate = compute_verifier(
        pepper,
        record.pepper_version,
        &record.salt,
        &ctx,
        &record.kdf,
        &indices,
    )?;
    Ok(candidate.ct_eq(&record.verifier).into())
}

fn valid_aspect(aspect: f64) -> bool {
    aspect.is_finite() && aspect > 0.0 && aspect <= MAX_ASPECT
}

fn valid_image_id(image_id: &str) -> bool {
    !image_id.is_empty() && image_id.len() <= MAX_IMAGE_ID_LEN
}

fn in_bounds(p: &Point, aspect: f64) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.x >= 0.0 && p.x <= 1.0 && p.y >= 0.0 && p.y <= aspect
}

/// Length-prefixed image id, then the bit patterns of r and aspect. Callers
/// cap the id at [`MAX_IMAGE_ID_LEN`], so the length always fits.
fn context(image_id: &str, r: f64, aspect: f64) -> Vec<u8> {
    let id = image_id.as_bytes();
    let mut out = Vec::with_capacity(id.len().saturating_add(20));
    out.extend_from_slice(&u32::try_from(id.len()).unwrap_or(u32::MAX).to_be_bytes());
    out.extend_from_slice(id);
    out.extend_from_slice(&r.to_bits().to_be_bytes());
    out.extend_from_slice(&aspect.to_bits().to_be_bytes());
    out
}

/// A count byte, then each index. Callers cap the count at [`MAX_TAPS`], so
/// it always fits.
fn encode_indices(indices: &[CellIndex]) -> Vec<u8> {
    let mut out = Vec::with_capacity(indices.len().saturating_mul(8).saturating_add(1));
    out.push(u8::try_from(indices.len()).unwrap_or(u8::MAX));
    for i in indices {
        out.extend_from_slice(&i.encode());
    }
    out
}

fn xor4(a: [u8; 4], b: [u8; 4]) -> [u8; 4] {
    let [a0, a1, a2, a3] = a;
    let [b0, b1, b2, b3] = b;
    [a0 ^ b0, a1 ^ b1, a2 ^ b2, a3 ^ b3]
}

/// Keystream for the offset of the tap that follows `earlier`. The count of
/// earlier indices is part of the message, so each position gets its own key.
fn offset_keystream(
    pepper: &dyn PepperProvider,
    pepper_version: u32,
    salt: &[u8; 16],
    ctx: &[u8],
    earlier: &[CellIndex],
) -> Result<[u8; 4], Error> {
    let mut msg = Vec::new();
    msg.extend_from_slice(DOMAIN_OFFSET);
    msg.push(0);
    msg.extend_from_slice(salt);
    msg.extend_from_slice(ctx);
    msg.extend_from_slice(&encode_indices(earlier));
    let [k0, k1, k2, k3, ..] = pepper.hmac(pepper_version, &msg)?;
    Ok([k0, k1, k2, k3])
}

fn compute_verifier(
    pepper: &dyn PepperProvider,
    pepper_version: u32,
    salt: &[u8; 16],
    ctx: &[u8],
    kdf: &KdfParams,
    indices: &[CellIndex],
) -> Result<[u8; 32], Error> {
    let mut password = Vec::new();
    password.extend_from_slice(DOMAIN_KDF);
    password.push(0);
    password.extend_from_slice(ctx);
    password.extend_from_slice(&encode_indices(indices));

    let params =
        Params::new(kdf.m_kib, kdf.t, kdf.p, Some(32)).map_err(|e| Error::Kdf(e.to_string()))?;
    let mut stretched = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(&password, salt, &mut stretched)
        .map_err(|e| Error::Kdf(e.to_string()))?;

    let mut msg = Vec::new();
    msg.extend_from_slice(DOMAIN_FINAL);
    msg.push(0);
    msg.extend_from_slice(&stretched);
    pepper.hmac(pepper_version, &msg)
}
