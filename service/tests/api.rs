//! End-to-end tests of the HTTP API, in memory, with cheap Argon2id settings.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use pixel_factor_core::KdfParams;
use pixel_factor_service::config::Config;
use pixel_factor_service::store::{Failures, Store};
use pixel_factor_service::{router, App, Lockout, MAX_BODY};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceExt;

const TOKEN: &str = "test-token";

fn env() -> HashMap<&'static str, String> {
    HashMap::from([
        ("PF_DB_PATH", ":memory:".to_owned()),
        ("PF_API_TOKEN", TOKEN.to_owned()),
        ("PF_IMAGES", "harbour=0.75,meadow=1.0".to_owned()),
        ("PF_DEV_PEPPER_HEX", "11".repeat(32)),
    ])
}

fn config_from(vars: &HashMap<&'static str, String>) -> Result<Config, String> {
    Config::from_lookup(|name| vars.get(name).cloned())
}

fn app() -> Router {
    let config = config_from(&env()).unwrap();
    let kdf = KdfParams {
        m_kib: 64,
        t: 1,
        p: 1,
    };
    let app = App::new(&config, Store::open_in_memory().unwrap(), kdf).unwrap();
    router(Arc::new(app))
}

fn five() -> Value {
    json!([
        [0.10, 0.10],
        [0.30, 0.20],
        [0.50, 0.30],
        [0.70, 0.40],
        [0.90, 0.50]
    ])
}

fn wrong() -> Value {
    json!([
        [0.10, 0.10],
        [0.30, 0.20],
        [0.50, 0.30],
        [0.70, 0.40],
        [0.90, 0.60]
    ])
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    body: Option<Value>,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    let body = match body {
        Some(b) => {
            req = req.header("content-type", "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let res = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
    (status, value)
}

async fn post(app: &Router, path: &str, body: Value) -> (StatusCode, Value) {
    call(app, "POST", path, Some(body), Some(TOKEN)).await
}

async fn enrol_alice(app: &Router) {
    let (status, body) = post(
        app,
        "/v1/enrol",
        json!({"user_id": "alice", "image_id": "harbour", "taps": five()}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

async fn verify(app: &Router, user: &str, taps: Value) -> Value {
    let (status, body) = post(app, "/v1/verify", json!({"user_id": user, "taps": taps})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

#[tokio::test]
async fn enrol_then_verify() {
    let app = app();
    enrol_alice(&app).await;
    assert_eq!(
        verify(&app, "alice", five()).await,
        json!({"match": true, "locked": false})
    );
    assert_eq!(
        verify(&app, "alice", wrong()).await,
        json!({"match": false, "locked": false})
    );
}

#[tokio::test]
async fn unknown_users_look_like_known_ones() {
    let app = app();
    enrol_alice(&app).await;
    let (status, known) = post(&app, "/v1/challenge", json!({"user_id": "alice"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(known, json!({"image_id": "harbour"}));

    let (status, first) = post(&app, "/v1/challenge", json!({"user_id": "nobody"})).await;
    assert_eq!(status, StatusCode::OK);
    let (_, again) = post(&app, "/v1/challenge", json!({"user_id": "nobody"})).await;
    assert_eq!(first, again, "an unknown user's image is stable");
    let image = first["image_id"].as_str().unwrap();
    assert!(image == "harbour" || image == "meadow");

    assert_eq!(
        verify(&app, "nobody", five()).await,
        json!({"match": false, "locked": false})
    );
}

#[tokio::test]
async fn decoy_images_are_spread_over_the_images() {
    let app = app();
    let mut seen = std::collections::BTreeSet::new();
    for i in 0..32 {
        let (_, body) = post(
            &app,
            "/v1/challenge",
            json!({"user_id": format!("user-{i}")}),
        )
        .await;
        seen.insert(body["image_id"].as_str().unwrap().to_owned());
    }
    assert_eq!(seen.len(), 2);
}

#[tokio::test]
async fn repeated_failures_lock_then_success_is_refused_while_locked() {
    let app = app();
    enrol_alice(&app).await;
    for _ in 0..4 {
        assert_eq!(
            verify(&app, "alice", wrong()).await,
            json!({"match": false, "locked": false})
        );
    }
    assert_eq!(
        verify(&app, "alice", wrong()).await,
        json!({"match": false, "locked": true})
    );
    assert_eq!(
        verify(&app, "alice", five()).await,
        json!({"match": false, "locked": true})
    );
}

#[tokio::test]
async fn success_resets_the_failure_count() {
    let app = app();
    enrol_alice(&app).await;
    for _ in 0..4 {
        verify(&app, "alice", wrong()).await;
    }
    assert_eq!(verify(&app, "alice", five()).await["match"], true);
    for _ in 0..4 {
        assert_eq!(verify(&app, "alice", wrong()).await["locked"], false);
    }
}

#[tokio::test]
async fn re_enrolment_clears_a_lock() {
    let app = app();
    enrol_alice(&app).await;
    for _ in 0..5 {
        verify(&app, "alice", wrong()).await;
    }
    assert_eq!(verify(&app, "alice", five()).await["locked"], true);
    enrol_alice(&app).await;
    assert_eq!(
        verify(&app, "alice", five()).await,
        json!({"match": true, "locked": false})
    );
}

#[tokio::test]
async fn delete_removes_the_user() {
    let app = app();
    enrol_alice(&app).await;
    let (status, _) = call(&app, "DELETE", "/v1/users/alice", None, Some(TOKEN)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(verify(&app, "alice", five()).await["match"], false);
    let (status, _) = call(&app, "DELETE", "/v1/users/alice", None, Some(TOKEN)).await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "deleting twice is not an error"
    );
}

#[tokio::test]
async fn unacceptable_enrolments_are_422_with_a_reason() {
    let app = app();
    let cases = [
        json!({"user_id": "a", "image_id": "nowhere", "taps": five()}),
        json!({"user_id": "a", "image_id": "harbour", "taps": [[0.1, 0.1], [0.3, 0.2]]}),
        json!({"user_id": "a", "image_id": "harbour", "taps": [[0.1, 0.1], [0.3, 0.2], [0.5, 0.3], [0.7, 0.4], [0.9, 0.9]]}),
        json!({"user_id": "a", "image_id": "harbour",
               "taps": [[0.1, 0.1], [0.101, 0.1], [0.5, 0.3], [0.7, 0.4], [0.9, 0.5]]}),
        json!({"user_id": "", "image_id": "harbour", "taps": five()}),
        json!({"user_id": "x".repeat(256), "image_id": "harbour", "taps": five()}),
    ];
    for case in cases {
        let (status, body) = post(&app, "/v1/enrol", case.clone()).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{case}");
        assert!(body["error"].is_string(), "{body}");
    }
}

#[tokio::test]
async fn close_points_warn_but_enrol() {
    let app = app();
    let taps = json!([
        [0.10, 0.10],
        [0.16, 0.10],
        [0.50, 0.30],
        [0.70, 0.40],
        [0.90, 0.50]
    ]);
    let (status, body) = post(
        &app,
        "/v1/enrol",
        json!({"user_id": "a", "image_id": "harbour", "taps": taps}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["warnings"].as_array().unwrap().len(), 1, "{body}");
}

#[tokio::test]
async fn any_tap_count_is_just_a_mismatch() {
    let app = app();
    enrol_alice(&app).await;
    for taps in [
        json!([]),
        json!([[0.5, 0.5]]),
        json!(vec![[0.5, 0.5]; 33]),
        json!(vec![[2.0, 0.5]; 5]),
    ] {
        assert_eq!(verify(&app, "alice", taps).await["match"], false);
    }
}

#[tokio::test]
async fn the_token_is_required_on_v1_only() {
    let app = app();
    for token in [None, Some("wrong"), Some("test-toke"), Some("test-token-")] {
        let (status, _) = call(
            &app,
            "POST",
            "/v1/challenge",
            Some(json!({"user_id": "a"})),
            token,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}");
    }
    let (status, body) = call(&app, "GET", "/healthz", None, None).await;
    assert_eq!((status, body), (StatusCode::OK, json!("ok")));
    let (status, body) = call(&app, "GET", "/version", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert!(body["commit"].is_string());
}

#[tokio::test]
async fn bodies_are_bounded_and_strict() {
    let app = app();
    let big = json!({"user_id": "a", "taps": [], "pad": "x".repeat(MAX_BODY)});
    let (status, _) = post(&app, "/v1/verify", big).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    let (status, _) = post(
        &app,
        "/v1/verify",
        json!({"user_id": "a", "taps": [], "extra": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[test]
fn lockout_doubles_up_to_the_cap() {
    let l = Lockout {
        max_failures: 5,
        base_secs: 60,
        max_secs: 3600,
        window_secs: 86_400,
    };
    assert_eq!(l.until(4, 1000), 0);
    assert_eq!(l.until(5, 1000), 1060);
    assert_eq!(l.until(6, 1000), 1120);
    assert_eq!(l.until(7, 1000), 1240);
    assert_eq!(l.until(11, 1000), 4600);
    assert_eq!(l.until(u32::MAX, 1000), 4600);
    assert_eq!(l.until(5, u64::MAX), u64::MAX);
}

#[test]
fn config_reads_the_environment() {
    let config = config_from(&env()).unwrap();
    assert_eq!(config.listen.to_string(), "127.0.0.1:0");
    assert_eq!(config.images.len(), 2);
    assert_eq!(
        config.lockout,
        Lockout {
            max_failures: 5,
            base_secs: 60,
            max_secs: 3600,
            window_secs: 86_400
        }
    );
    assert!(config.max_concurrent >= 1);

    let mut vars = env();
    vars.insert("PF_LISTEN", "0.0.0.0:8080".to_owned());
    vars.insert("PF_MAX_FAILURES", "3".to_owned());
    vars.insert("PF_LOCKOUT_BASE_SECS", "10".to_owned());
    vars.insert("PF_LOCKOUT_MAX_SECS", "20".to_owned());
    vars.insert("PF_FAILURE_WINDOW_SECS", "30".to_owned());
    vars.insert("PF_MAX_CONCURRENT", "0".to_owned());
    let config = config_from(&vars).unwrap();
    assert_eq!(config.listen.port(), 8080);
    assert_eq!(
        config.lockout,
        Lockout {
            max_failures: 3,
            base_secs: 10,
            max_secs: 20,
            window_secs: 30
        }
    );
    assert_eq!(config.max_concurrent, 1);
}

#[test]
fn config_rejects_bad_values() {
    let bad: [(&str, Option<&str>); 15] = [
        ("PF_DB_PATH", None),
        ("PF_API_TOKEN", Some("")),
        ("PF_IMAGES", None),
        ("PF_IMAGES", Some(",")),
        ("PF_IMAGES", Some("harbour")),
        ("PF_IMAGES", Some("=0.75")),
        ("PF_IMAGES", Some("harbour=wide")),
        ("PF_IMAGES", Some("harbour=0")),
        ("PF_IMAGES", Some("harbour=17")),
        ("PF_IMAGES", Some("a=1,a=1")),
        ("PF_DEV_PEPPER_HEX", Some("zz")),
        ("PF_DEV_PEPPER_HEX", Some("1111")),
        ("PF_MAX_FAILURES", Some("0")),
        ("PF_LISTEN", Some("nowhere")),
        ("PF_FAILURE_WINDOW_SECS", Some("3599")),
    ];
    for (name, value) in bad {
        let mut vars = env();
        match value {
            Some(v) => vars.insert(name, v.to_owned()),
            None => vars.remove(name),
        };
        assert!(config_from(&vars).is_err(), "{name}={value:?}");
    }
    let mut vars = env();
    vars.insert("PF_IMAGES", format!("{}=1", "x".repeat(256)));
    assert!(config_from(&vars).is_err());
}

#[tokio::test]
async fn unknown_users_are_locked_like_known_ones() {
    // Otherwise five failures would show whether an account exists.
    let app = app();
    enrol_alice(&app).await;
    for user in ["alice", "nobody"] {
        for _ in 0..4 {
            assert_eq!(
                verify(&app, user, wrong()).await,
                json!({"match": false, "locked": false}),
                "{user}"
            );
        }
        assert_eq!(
            verify(&app, user, wrong()).await,
            json!({"match": false, "locked": true}),
            "{user}"
        );
        assert_eq!(
            verify(&app, user, five()).await,
            json!({"match": false, "locked": true}),
            "{user}"
        );
    }
}

fn record() -> pixel_factor_core::Record {
    let config = config_from(&env()).unwrap();
    let kdf = KdfParams {
        m_kib: 64,
        t: 1,
        p: 1,
    };
    let pepper = pixel_factor_core::StaticPepper::new(1, config.dev_pepper.clone());
    let taps: Vec<_> = [0.1, 0.3, 0.5, 0.7, 0.9]
        .into_iter()
        .map(|x| pixel_factor_core::geometry::Point::new(x, 0.3))
        .collect();
    let policy = pixel_factor_core::Policy {
        kdf,
        ..pixel_factor_core::Policy::standard(0.75)
    };
    pixel_factor_core::enrol(&taps, "harbour", &policy, &pepper)
        .unwrap()
        .record
}

#[test]
fn store_keeps_records_across_reopening() {
    let record = record();
    let dir = std::env::temp_dir().join(format!("pf-store-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.db");
    {
        let store = Store::open(&path).unwrap();
        assert!(store.get("alice").unwrap().is_none());
        store.put("alice", &record).unwrap();
        store.add_failure("alice", 100, 50).unwrap();
        store.set_locked_until("alice", 160).unwrap();
    }
    let store = Store::open(&path).unwrap();
    assert_eq!(store.get("alice").unwrap(), Some(record.clone()));
    assert_eq!(
        store.failures("alice", 100, 50).unwrap(),
        Failures {
            count: 1,
            locked_until: 160
        }
    );
    store.put("alice", &record).unwrap();
    assert_eq!(
        store.failures("alice", 100, 50).unwrap(),
        Failures::default(),
        "enrolment clears failures"
    );
    store.add_failure("alice", 100, 50).unwrap();
    store.delete("alice").unwrap();
    assert!(store.get("alice").unwrap().is_none());
    assert_eq!(
        store.failures("alice", 100, 50).unwrap(),
        Failures::default(),
        "deletion clears failures"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn failures_expire_after_the_window() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.add_failure("x", 100, 50).unwrap(), 1);
    assert_eq!(
        store.add_failure("x", 150, 50).unwrap(),
        2,
        "exactly at the window edge still counts"
    );
    store.set_locked_until("x", 160).unwrap();
    assert_eq!(
        store.failures("x", 200, 50).unwrap(),
        Failures {
            count: 2,
            locked_until: 160
        }
    );
    assert_eq!(store.failures("x", 201, 50).unwrap(), Failures::default());
    assert_eq!(
        store.add_failure("x", 201, 50).unwrap(),
        1,
        "an expired count starts again"
    );
    assert_eq!(
        store.failures("x", 201, 50).unwrap(),
        Failures {
            count: 1,
            locked_until: 0
        }
    );
}

#[test]
fn pruning_drops_only_expired_unlocked_rows() {
    let store = Store::open_in_memory().unwrap();
    store.add_failure("old", 100, 50).unwrap();
    store.add_failure("old-locked", 100, 50).unwrap();
    store.set_locked_until("old-locked", 300).unwrap();
    store.add_failure("recent", 180, 50).unwrap();
    assert_eq!(store.prune_failures(200, 50).unwrap(), 1);
    assert_eq!(store.failures("old-locked", 200, 200).unwrap().count, 1);
    assert_eq!(store.failures("recent", 200, 50).unwrap().count, 1);
    assert_eq!(
        store.failures("old", 200, 200).unwrap(),
        Failures::default()
    );
}

#[tokio::test]
async fn too_few_distinct_points_are_rejected() {
    let app = app();
    // The fifth tap returns to the first point, leaving four distinct points.
    let taps = json!([
        [0.10, 0.10],
        [0.30, 0.20],
        [0.50, 0.30],
        [0.70, 0.40],
        [0.10, 0.10]
    ]);
    let (status, body) = post(
        &app,
        "/v1/enrol",
        json!({"user_id": "a", "image_id": "harbour", "taps": taps}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        body["error"].as_str().unwrap().contains("distinct"),
        "{body}"
    );
}

#[tokio::test]
async fn internal_errors_are_generic_500s() {
    let dir = std::env::temp_dir().join(format!("pf-corrupt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.db");
    drop(Store::open(&path).unwrap());
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "INSERT INTO users (user_id, record) VALUES ('bob', 'not json')",
        [],
    )
    .unwrap();
    drop(conn);

    let config = config_from(&env()).unwrap();
    let kdf = KdfParams {
        m_kib: 64,
        t: 1,
        p: 1,
    };
    let app = router(Arc::new(
        App::new(&config, Store::open(&path).unwrap(), kdf).unwrap(),
    ));
    let requests = [
        ("/v1/challenge", json!({"user_id": "bob"})),
        ("/v1/verify", json!({"user_id": "bob", "taps": five()})),
    ];
    for (path, body) in requests {
        let (status, answer) = post(&app, path, body).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{path}");
        assert_eq!(
            answer,
            json!({"error": "internal error"}),
            "nothing about the cause leaks"
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
