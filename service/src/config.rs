//! Configuration from the environment. See `docs/service.md` for each variable.

use crate::Lockout;
use pixel_factor_core::geometry::MAX_ASPECT;
use pixel_factor_core::MAX_IMAGE_ID_LEN;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Shortest development pepper accepted, in bytes.
const MIN_PEPPER_LEN: usize = 32;

/// Everything the service reads from the environment.
#[derive(Clone, Debug)]
pub struct Config {
    /// Address to bind. Port 0 picks a free one.
    pub listen: SocketAddr,
    /// SQLite database file.
    pub db_path: PathBuf,
    /// Bearer token every `/v1` request must carry.
    pub api_token: String,
    /// Curated images: id to aspect (height over width).
    pub images: BTreeMap<String, f64>,
    /// Development pepper key.
    pub dev_pepper: Vec<u8>,
    /// Lockout after repeated failures.
    pub lockout: Lockout,
    /// Enrolments and verifications allowed to run at once.
    pub max_concurrent: usize,
}

impl Config {
    /// Read the configuration from the process environment.
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Read the configuration through `lookup`, which returns a variable's
    /// value or `None` when it is unset.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let required = |name: &str| {
            lookup(name)
                .filter(|v| !v.is_empty())
                .ok_or_else(|| format!("{name} must be set"))
        };

        let listen = lookup("PF_LISTEN")
            .unwrap_or_else(|| "127.0.0.1:0".to_owned())
            .parse()
            .map_err(|e| format!("PF_LISTEN: {e}"))?;
        let db_path = PathBuf::from(required("PF_DB_PATH")?);
        let api_token = required("PF_API_TOKEN")?;
        let images = parse_images(&required("PF_IMAGES")?)?;
        let dev_pepper = hex::decode(required("PF_DEV_PEPPER_HEX")?)
            .map_err(|e| format!("PF_DEV_PEPPER_HEX: {e}"))?;
        if dev_pepper.len() < MIN_PEPPER_LEN {
            return Err(format!(
                "PF_DEV_PEPPER_HEX must be at least {MIN_PEPPER_LEN} bytes"
            ));
        }

        let number = |name: &str, default: u64| -> Result<u64, String> {
            lookup(name).map_or(Ok(default), |v| {
                v.parse().map_err(|e| format!("{name}: {e}"))
            })
        };
        let max_failures = u32::try_from(number("PF_MAX_FAILURES", 5)?)
            .map_err(|_| "PF_MAX_FAILURES is too large".to_owned())?;
        if max_failures == 0 {
            return Err("PF_MAX_FAILURES must be at least 1".to_owned());
        }
        let lockout = Lockout {
            max_failures,
            base_secs: number("PF_LOCKOUT_BASE_SECS", 60)?,
            max_secs: number("PF_LOCKOUT_MAX_SECS", 3600)?,
            window_secs: number("PF_FAILURE_WINDOW_SECS", 86_400)?,
        };
        if lockout.window_secs < lockout.max_secs {
            return Err("PF_FAILURE_WINDOW_SECS must be at least PF_LOCKOUT_MAX_SECS".to_owned());
        }
        let cpus = std::thread::available_parallelism().map_or(1, usize::from);
        let max_concurrent = usize::try_from(number(
            "PF_MAX_CONCURRENT",
            u64::try_from(cpus).unwrap_or(1),
        )?)
        .map_err(|_| "PF_MAX_CONCURRENT is too large".to_owned())?
        .max(1);

        Ok(Self {
            listen,
            db_path,
            api_token,
            images,
            dev_pepper,
            lockout,
            max_concurrent,
        })
    }
}

/// Parse `id=aspect,id=aspect`.
fn parse_images(spec: &str) -> Result<BTreeMap<String, f64>, String> {
    let mut images = BTreeMap::new();
    for entry in spec.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let (id, aspect) = entry
            .split_once('=')
            .ok_or_else(|| format!("PF_IMAGES: '{entry}' is not id=aspect"))?;
        if id.is_empty() || id.len() > MAX_IMAGE_ID_LEN {
            return Err(format!(
                "PF_IMAGES: image id '{id}' must be 1 to {MAX_IMAGE_ID_LEN} bytes"
            ));
        }
        let aspect: f64 = aspect
            .parse()
            .map_err(|e| format!("PF_IMAGES: aspect of '{id}': {e}"))?;
        if !(aspect.is_finite() && aspect > 0.0 && aspect <= MAX_ASPECT) {
            return Err(format!(
                "PF_IMAGES: aspect of '{id}' must be in (0, {MAX_ASPECT}]"
            ));
        }
        if images.insert(id.to_owned(), aspect).is_some() {
            return Err(format!("PF_IMAGES: image '{id}' is listed twice"));
        }
    }
    if images.is_empty() {
        return Err("PF_IMAGES must list at least one image".to_owned());
    }
    Ok(images)
}
