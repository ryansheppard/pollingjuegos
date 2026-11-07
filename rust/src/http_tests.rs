//! Drives the real route table so middleware, extractors and JSON shapes are
//! exercised together; the per-module tests only cover the SQL layer.

use std::sync::Arc;

use cookie::Cookie;
use ntex::http::{Request, Response, StatusCode, header};
use ntex::web::{self, test};
use serde_json::{Value, json};

use crate::accounts::pkce_challenge;
use crate::auth::{self, SESSION_COOKIE};
use crate::ballots;
use crate::cache::ResponseCache;
use crate::config::Config;
use crate::db::{self, Db};
use crate::nfl_stats::{NflStats, QbStats, normalize_name};
use crate::{AppData, St};

const ORIGIN: &str = "https://pj.example";

struct Harness {
    db: Db,
    user_id: i64,
    token: String,
    // Shared across calls the way one worker's are, so caching is exercised.
    nfl: NflStats,
    cache: ResponseCache,
}

impl Harness {
    fn new() -> Self {
        Self::with_db(Db::new(db::test_conn()))
    }

    fn with_db(db: Db) -> Self {
        let (user_id, token) = {
            let conn = db.conn();
            conn.execute_batch(
                "INSERT INTO approved_discord_users (discord_id) VALUES ('100');
                 INSERT INTO poll_weeks (season, week, closes_at) VALUES
                     (2025, 18, '2026-01-05T00:00:00Z'),
                     (2026, 2, NULL),
                     (2026, 1, '2099-01-01T00:00:00Z');",
            )
            .unwrap();
            let user_id = auth::upsert_user(&conn, "100", "voter").unwrap();
            (user_id, auth::create_session(&conn, user_id).unwrap())
        };
        Self {
            db,
            user_id,
            token,
            nfl: NflStats::default(),
            cache: ResponseCache::default(),
        }
    }

    async fn call(&self, req: Request) -> Response {
        let st = web::AppState::new(AppData {
            db: self.db.clone(),
            cfg: Arc::new(Config {
                bind_addr: "127.0.0.1:0".into(),
                db_path: ":memory:".into(),
                public_origin: ORIGIN.into(),
                discord_client_id: "client-id".into(),
                discord_client_secret: "client-secret".into(),
            }),
            http: ntex::client::Client::new(),
            nfl: self.nfl.clone(),
            cache: self.cache.clone(),
        });
        let app = test::init_service_st(st, web::App::<St>::new().configure(crate::routes)).await;
        test::call_service(&app, req).await
    }

    fn session(&self) -> Cookie<'static> {
        Cookie::new(SESSION_COOKIE, self.token.clone())
    }

    fn get(&self, uri: &str) -> test::TestRequest {
        test::TestRequest::get().uri(uri).cookie(self.session())
    }

    fn put_ballot(&self, uri: &str, ids: &[i64]) -> test::TestRequest {
        test::TestRequest::put()
            .uri(uri)
            .cookie(self.session())
            .header(header::ORIGIN, ORIGIN)
            .json(&json!({ "quarterback_ids": ids }))
    }

    fn bearer(&self, req: test::TestRequest, token: &str) -> Request {
        req.header(header::AUTHORIZATION, format!("Bearer {token}"))
            .to_request()
    }

    fn qb_ids(&self, n: i64) -> Vec<i64> {
        self.db
            .conn()
            .prepare("SELECT id FROM quarterbacks ORDER BY id LIMIT ?1")
            .unwrap()
            .query_map([n], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }
}

async fn json_body(res: Response) -> Value {
    serde_json::from_slice(&test::read_body(res).await).expect("JSON body")
}

fn location(res: &Response) -> &str {
    res.headers()
        .get(header::LOCATION)
        .expect("Location header")
        .to_str()
        .unwrap()
}

/// Returns the `Set-Cookie` header for `name`, if the response set one.
fn set_cookie<'a>(res: &'a Response, name: &str) -> Option<&'a str> {
    res.headers()
        .get_all(header::SET_COOKIE)
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with(&format!("{name}=")))
}

fn oauth_cookie_value(res: &Response) -> String {
    set_cookie(res, "__Host-oauth")
        .and_then(|c| c.split(';').next())
        .and_then(|kv| kv.split_once('='))
        .map(|(_, v)| v.to_owned())
        .expect("oauth cookie")
}

#[ntex::test]
async fn api_requires_a_session() {
    let h = Harness::new();
    for uri in [
        "/api/me",
        "/api/weeks",
        "/api/quarterbacks",
        "/api/weeks/2026/1/ballot",
        "/api/weeks/2026/1/rankings",
    ] {
        let res = h.call(test::TestRequest::get().uri(uri).to_request()).await;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{uri}");
        assert_eq!(json_body(res).await, json!({ "detail": "Unauthorized" }));
    }
}

#[ntex::test]
async fn invalid_and_revoked_sessions_are_rejected() {
    let h = Harness::new();
    let garbage = test::TestRequest::get()
        .uri("/api/me")
        .cookie(Cookie::new(SESSION_COOKIE, "not-a-token"));
    assert_eq!(
        h.call(garbage.to_request()).await.status(),
        StatusCode::UNAUTHORIZED
    );

    let unknown = test::TestRequest::get()
        .uri("/api/me")
        .cookie(Cookie::new(SESSION_COOKIE, auth::new_token().0));
    assert_eq!(
        h.call(unknown.to_request()).await.status(),
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        h.call(h.get("/api/me").to_request()).await.status(),
        StatusCode::OK
    );
    h.db.conn()
        .execute("UPDATE approved_discord_users SET enabled = 0", [])
        .unwrap();
    assert_eq!(
        h.call(h.get("/api/me").to_request()).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

/// Handlers build bodies with `crate::json` rather than ntex's `.json()`, so the
/// header it used to set has to be checked here.
#[ntex::test]
async fn json_responses_declare_content_type() {
    let h = Harness::new();
    let ok = h.call(h.get("/api/weeks").to_request()).await;
    let unauthorized = h
        .call(test::TestRequest::get().uri("/api/me").to_request())
        .await;
    for (res, status) in [
        (ok, StatusCode::OK),
        (unauthorized, StatusCode::UNAUTHORIZED),
    ] {
        assert_eq!(res.status(), status);
        assert_eq!(
            res.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
    }
}

#[ntex::test]
async fn me_returns_the_session_user() {
    let h = Harness::new();
    let res = h.call(h.get("/api/me").to_request()).await;
    assert_eq!(json_body(res).await, json!({ "id": h.user_id }));
}

#[ntex::test]
async fn weeks_are_newest_first_with_closed_flag() {
    let h = Harness::new();
    let body = json_body(h.call(h.get("/api/weeks").to_request()).await).await;
    let got: Vec<(i64, i64, bool)> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|w| {
            (
                w["season"].as_i64().unwrap(),
                w["week"].as_i64().unwrap(),
                w["is_closed"].as_bool().unwrap(),
            )
        })
        .collect();
    // 2026/2 has no deadline; ordering by closes_at would have sunk it to the bottom.
    assert_eq!(got, [(2026, 2, false), (2026, 1, false), (2025, 18, true)]);
    assert_eq!(body[0]["closes_at"], Value::Null);
    assert_eq!(body[2]["closes_at"], "2026-01-05T00:00:00Z");
}

#[ntex::test]
async fn quarterbacks_shape_and_pagination() {
    let h = Harness::new();
    let total: i64 =
        h.db.conn()
            .query_row("SELECT count(*) FROM quarterbacks", [], |r| r.get(0))
            .unwrap();
    let total = usize::try_from(total).unwrap();

    let default_page = json_body(h.call(h.get("/api/quarterbacks").to_request()).await).await;
    let default_page = default_page.as_array().unwrap();
    assert_eq!(default_page.len(), 50.min(total));
    let qb = &default_page[0];
    assert!(qb["id"].is_i64() && qb["name"].is_string());
    assert!(qb["team"]["id"].is_i64());
    assert!(qb["team"]["name"].is_string());
    assert!(qb["team"]["abbreviation"].is_string());
    // The harness never loads nflverse data, so the key is present but null.
    assert_eq!(qb.get("stats"), Some(&Value::Null));

    // Mirrors the frontend's loadQuarterbacks loop, which stops on a short page.
    let mut seen = Vec::new();
    loop {
        let uri = format!("/api/quarterbacks?limit=100&offset={}", seen.len());
        let page = json_body(h.call(h.get(&uri).to_request()).await).await;
        let page = page.as_array().unwrap().clone();
        let len = page.len();
        seen.extend(page.into_iter().map(|qb| qb["id"].as_i64().unwrap()));
        if len < 100 {
            break;
        }
        assert!(seen.len() <= total, "pagination never terminated");
    }
    assert_eq!(seen.len(), total);
}

#[ntex::test]
async fn quarterbacks_search_and_bad_pages() {
    let h = Harness::new();
    let body = json_body(
        h.call(h.get("/api/quarterbacks?q=mahomes").to_request())
            .await,
    )
    .await;
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["name"], "Patrick Mahomes");
    assert_eq!(body[0]["team"]["abbreviation"], "KC");

    for uri in [
        "/api/quarterbacks?limit=0",
        "/api/quarterbacks?limit=101",
        "/api/quarterbacks?offset=-1",
    ] {
        let res = h.call(h.get(uri).to_request()).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(
            json_body(res).await,
            json!({ "detail": "limit must be 1-100 and offset nonnegative" })
        );
    }
}

#[ntex::test]
async fn ballot_round_trip_and_rankings() {
    let h = Harness::new();
    let ids = h.qb_ids(15);

    let empty = json_body(h.call(h.get("/api/weeks/2026/1/ballot").to_request()).await).await;
    assert_eq!(empty, json!({ "season": 2026, "week": 1, "entries": [] }));

    let res = h
        .call(h.put_ballot("/api/weeks/2026/1/ballot", &ids).to_request())
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let saved = json_body(res).await;
    assert_eq!(saved["entries"].as_array().unwrap().len(), 15);
    assert_eq!(saved["entries"][0]["rank"], 1);
    assert_eq!(saved["entries"][0]["quarterback"]["id"], ids[0]);
    assert!(saved["entries"][0]["quarterback"]["team"]["abbreviation"].is_string());

    let fetched = json_body(h.call(h.get("/api/weeks/2026/1/ballot").to_request()).await).await;
    assert_eq!(fetched, saved);

    let rankings = json_body(
        h.call(h.get("/api/weeks/2026/1/rankings").to_request())
            .await,
    )
    .await;
    assert_eq!(rankings["ballots"], 1);
    assert_eq!(rankings["rankings"].as_array().unwrap().len(), 10);
    let top = &rankings["rankings"][0];
    assert_eq!(
        (top["rank"].clone(), top["points"].clone()),
        (json!(1), json!(15))
    );
    assert_eq!(top["votes"], 1);
    assert_eq!(top["previous_rank"], Value::Null);
    assert_eq!(top["quarterback"]["id"], ids[0]);
}

#[ntex::test]
async fn ballot_errors_use_detail_bodies() {
    let h = Harness::new();
    let ids = h.qb_ids(15);

    let res = h
        .call(
            h.put_ballot("/api/weeks/2026/1/ballot", &ids[..14])
                .to_request(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(res).await,
        json!({ "detail": "A ballot must contain exactly 15 distinct quarterbacks" })
    );

    let res = h
        .call(h.put_ballot("/api/weeks/2025/18/ballot", &ids).to_request())
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(res).await,
        json!({ "detail": "Voting for this week has closed" })
    );

    for uri in ["/api/weeks/2026/9/ballot", "/api/weeks/2026/9/rankings"] {
        let res = h.call(h.get(uri).to_request()).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{uri}");
        assert_eq!(json_body(res).await, json!({ "detail": "Not Found" }));
    }
}

#[ntex::test]
async fn writes_require_same_origin() {
    let h = Harness::new();
    let ids = h.qb_ids(15);
    let body = json!({ "quarterback_ids": ids });
    let uri = "/api/weeks/2026/1/ballot";

    let cross = test::TestRequest::put()
        .uri(uri)
        .cookie(h.session())
        .header(header::ORIGIN, "https://evil.example")
        .json(&body);
    let res = h.call(cross.to_request()).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        json_body(res).await,
        json!({ "detail": "Cross-origin request rejected" })
    );

    let no_origin = test::TestRequest::put()
        .uri(uri)
        .cookie(h.session())
        .json(&body);
    assert_eq!(
        h.call(no_origin.to_request()).await.status(),
        StatusCode::FORBIDDEN
    );

    let fetch_site = test::TestRequest::put()
        .uri(uri)
        .cookie(h.session())
        .header("sec-fetch-site", "same-origin")
        .json(&body);
    assert_eq!(
        h.call(fetch_site.to_request()).await.status(),
        StatusCode::OK
    );

    let votes: i64 =
        h.db.conn()
            .query_row("SELECT count(*) FROM votes", [], |r| r.get(0))
            .unwrap();
    assert_eq!(votes, 15);
}

#[ntex::test]
async fn frontend_gate_redirects_to_login() {
    let h = Harness::new();
    let anon = h
        .call(test::TestRequest::get().uri("/auth/frontend/").to_request())
        .await;
    assert_eq!(anon.status(), StatusCode::FOUND);
    assert_eq!(location(&anon), "/accounts/discord/login/?next=%2F");

    let signed_in = h.call(h.get("/auth/frontend/").to_request()).await;
    assert_eq!(signed_in.status(), StatusCode::NO_CONTENT);
}

#[ntex::test]
async fn login_alias_keeps_only_local_next() {
    let h = Harness::new();
    let res = h
        .call(
            test::TestRequest::get()
                .uri("/accounts/login/?next=%2Fweeks%3Fa%3D1")
                .to_request(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::FOUND);
    assert_eq!(
        location(&res),
        "/accounts/discord/login/?next=%2Fweeks%3Fa%3D1"
    );

    let res = h
        .call(
            test::TestRequest::get()
                .uri("/accounts/login/?next=%2F%2Fevil.example")
                .to_request(),
        )
        .await;
    assert_eq!(location(&res), "/accounts/discord/login/?next=%2F");
}

#[ntex::test]
async fn discord_login_sets_state_cookie() {
    let h = Harness::new();
    let res = h
        .call(
            test::TestRequest::get()
                .uri("/accounts/discord/login/")
                .to_request(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::FOUND);
    let loc = location(&res);
    assert!(
        loc.starts_with("https://discord.com/oauth2/authorize?"),
        "{loc}"
    );
    for part in [
        "client_id=client-id",
        "scope=identify",
        "response_type=code",
        "redirect_uri=https%3A%2F%2Fpj.example%2Faccounts%2Fdiscord%2Flogin%2Fcallback%2F",
        "state=",
        "code_challenge=",
        "code_challenge_method=S256",
    ] {
        assert!(loc.contains(part), "{loc} missing {part}");
    }
    let cookie = set_cookie(&res, "__Host-oauth").expect("oauth cookie");
    for attr in ["HttpOnly", "Secure", "Path=/", "SameSite=Lax"] {
        assert!(cookie.contains(attr), "{cookie} missing {attr}");
    }
}

#[ntex::test]
async fn callback_rejects_missing_or_mismatched_state() {
    let h = Harness::new();

    let res = h
        .call(
            test::TestRequest::get()
                .uri("/accounts/discord/login/callback/?code=x&state=y")
                .to_request(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(
        set_cookie(&res, "__Host-oauth").is_some(),
        "state cookie must be cleared"
    );

    let login = h
        .call(
            test::TestRequest::get()
                .uri("/accounts/discord/login/")
                .to_request(),
        )
        .await;
    let oauth_value = oauth_cookie_value(&login);
    let res = h
        .call(
            test::TestRequest::get()
                .uri("/accounts/discord/login/callback/?code=x&state=wrong")
                .cookie(Cookie::new("__Host-oauth", oauth_value))
                .to_request(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let cancelled = h
        .call(
            test::TestRequest::get()
                .uri("/accounts/discord/login/callback/?error=access_denied")
                .to_request(),
        )
        .await;
    assert_eq!(cancelled.status(), StatusCode::FOUND);
    assert_eq!(location(&cancelled), "/");
}

#[ntex::test]
async fn logout_requires_same_origin_and_ends_session() {
    let h = Harness::new();

    let cross = test::TestRequest::post()
        .uri("/accounts/logout/")
        .cookie(h.session())
        .header(header::ORIGIN, "https://evil.example");
    assert_eq!(
        h.call(cross.to_request()).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        h.call(h.get("/api/me").to_request()).await.status(),
        StatusCode::OK
    );

    let res = h
        .call(
            test::TestRequest::post()
                .uri("/accounts/logout/")
                .cookie(h.session())
                .header(header::ORIGIN, ORIGIN)
                .to_request(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&res), "/");
    assert!(
        set_cookie(&res, SESSION_COOKIE)
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        h.call(h.get("/api/me").to_request()).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

/// The cached unfiltered list must page exactly like the SQL path it replaces.
#[ntex::test]
async fn cached_quarterback_pages_match_the_database() {
    let h = Harness::new();
    let total = ballots::all_quarterbacks(&h.db.conn()).unwrap().len();
    for (limit, offset) in [(50, 0), (100, 0), (10, 85), (7, total - 3), (5, total + 10)] {
        let expected: Value = {
            let conn = h.db.conn();
            let page = ballots::list_quarterbacks(&conn, "", None, limit, offset as i64).unwrap();
            serde_json::to_value(
                page.into_iter()
                    .map(|quarterback| crate::models::QuarterbackWithStats {
                        stats: None,
                        quarterback,
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap()
        };
        let uri = format!("/api/quarterbacks?limit={limit}&offset={offset}");
        // The second request is served from the entry the first one built.
        for _ in 0..2 {
            let got = json_body(h.call(h.get(&uri).to_request()).await).await;
            assert_eq!(got, expected, "{uri}");
        }
    }
    // Filtered requests bypass the cache but must still validate pages.
    let res = h
        .call(h.get("/api/quarterbacks?limit=101").to_request())
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[ntex::test]
async fn cached_quarterbacks_follow_stats_refresh() {
    let h = Harness::new();
    let first = json_body(
        h.call(h.get("/api/quarterbacks?limit=100").to_request())
            .await,
    )
    .await;
    let name = first[0]["name"].as_str().unwrap().to_owned();
    assert_eq!(first[0]["stats"], Value::Null);

    let stats = QbStats {
        season: 2026,
        games: 8,
        dropbacks: 300,
        epa: Some(12.5),
        epa_per_dropback: Some(0.04),
        cpoe: Some(1.5),
        qbr: Some(61.0),
        qualified: true,
        qualified_count: 32,
        qbr_rank: Some(9),
        epa_rank: Some(11),
        cpoe_rank: Some(7),
    };
    h.nfl
        .replace([(normalize_name(&name), stats)].into_iter().collect());

    let refreshed = json_body(
        h.call(h.get("/api/quarterbacks?limit=100").to_request())
            .await,
    )
    .await;
    assert_eq!(refreshed[0]["name"], name.as_str());
    assert_eq!(refreshed[0]["stats"]["qbr_rank"], 9);
}

/// Each worker has its own connection, so the cache must notice writes from
/// this connection (`total_changes`) and from any other (`data_version`).
#[ntex::test]
async fn cached_rankings_follow_writes_from_any_connection() {
    let dir = std::env::var_os("TEST_TMPDIR").map_or_else(std::env::temp_dir, Into::into);
    let path = dir.join(format!("pj-rankings-cache-{}.sqlite3", std::process::id()));
    let path_str = path.to_str().unwrap().to_owned();
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{path_str}{suffix}"));
    }
    db::migrate(path_str.clone()).unwrap();
    let h = Harness::with_db(Db::new(db::open_db(path_str.clone()).unwrap()));
    let ballots_now = async || {
        json_body(
            h.call(h.get("/api/weeks/2026/1/rankings").to_request())
                .await,
        )
        .await["ballots"]
            .as_i64()
            .unwrap()
    };

    assert_eq!(ballots_now().await, 0);
    assert_eq!(ballots_now().await, 0);

    let ids = h.qb_ids(15);
    let res = h
        .call(h.put_ballot("/api/weeks/2026/1/ballot", &ids).to_request())
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(ballots_now().await, 1, "own-connection write");

    let other = db::open_db(path_str.clone()).unwrap();
    let voter = auth::upsert_user(&other, "200", "other").unwrap();
    ballots::submit_ballot(&other, 2026, 1, voter, &ids).unwrap();
    assert_eq!(ballots_now().await, 2, "write from another connection");

    other
        .execute("DELETE FROM ballots WHERE voter_id = ?1", [voter])
        .unwrap();
    assert_eq!(ballots_now().await, 1, "hand-written delete elsewhere");

    drop(other);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{path_str}{suffix}"));
    }
}

const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

impl Harness {
    fn login_code(&self) -> String {
        auth::create_login_code(&self.db.conn(), self.user_id, &pkce_challenge(VERIFIER)).unwrap()
    }

    async fn exchange(&self, code: &str, verifier: &str) -> Response {
        self.call(
            test::TestRequest::post()
                .uri("/accounts/desktop/token/")
                .json(&json!({ "code": code, "code_verifier": verifier }))
                .to_request(),
        )
        .await
    }
}

#[ntex::test]
async fn bearer_sessions_skip_the_origin_check() {
    let h = Harness::new();
    let ids = h.qb_ids(15);
    let put = || {
        test::TestRequest::put()
            .uri("/api/weeks/2026/1/ballot")
            .json(&json!({ "quarterback_ids": ids }))
    };

    let res = h.call(h.bearer(put(), &h.token)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let cross = put().header(header::ORIGIN, "https://evil.example");
    assert_eq!(
        h.call(h.bearer(cross, &h.token)).await.status(),
        StatusCode::OK
    );

    // A bearer header that doesn't resolve must not fall back to the cookie.
    let with_cookie = put().cookie(h.session());
    assert_eq!(
        h.call(h.bearer(with_cookie, "not-a-token")).await.status(),
        StatusCode::UNAUTHORIZED
    );

    h.db.conn()
        .execute("UPDATE approved_discord_users SET enabled = 0", [])
        .unwrap();
    let me = test::TestRequest::get().uri("/api/me");
    assert_eq!(
        h.call(h.bearer(me, &h.token)).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[ntex::test]
async fn desktop_login_carries_loopback_port_and_challenge() {
    let h = Harness::new();
    let challenge = pkce_challenge(VERIFIER);
    let res = h
        .call(
            test::TestRequest::get()
                .uri(&format!(
                    "/accounts/discord/login/?desktop_port=53123&desktop_challenge={challenge}"
                ))
                .to_request(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::FOUND);
    // Discord still redirects to the server; only the server knows about the app.
    assert!(location(&res).contains(
        "redirect_uri=https%3A%2F%2Fpj.example%2Faccounts%2Fdiscord%2Flogin%2Fcallback%2F"
    ));

    let cancelled = h
        .call(
            test::TestRequest::get()
                .uri("/accounts/discord/login/callback/?error=access_denied")
                .cookie(Cookie::new("__Host-oauth", oauth_cookie_value(&res)))
                .to_request(),
        )
        .await;
    assert_eq!(cancelled.status(), StatusCode::FOUND);
    assert_eq!(
        location(&cancelled),
        "http://127.0.0.1:53123/callback?error=access_denied"
    );

    for query in [
        "desktop_port=53123".to_owned(),
        format!("desktop_challenge={challenge}"),
        format!("desktop_port=0&desktop_challenge={challenge}"),
        "desktop_port=53123&desktop_challenge=plain".to_owned(),
    ] {
        let res = h
            .call(
                test::TestRequest::get()
                    .uri(&format!("/accounts/discord/login/?{query}"))
                    .to_request(),
            )
            .await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{query}");
    }
}

#[ntex::test]
async fn desktop_code_exchanges_once_for_a_bearer_session() {
    let h = Harness::new();
    let code = h.login_code();

    let res = h.exchange(&code, VERIFIER).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["expires_in"], auth::SESSION_TTL_SECS);
    let token = body["access_token"].as_str().unwrap().to_owned();

    let me = test::TestRequest::get().uri("/api/me");
    let res = h.call(h.bearer(me, &token)).await;
    assert_eq!(json_body(res).await, json!({ "id": h.user_id }));

    let replay = h.exchange(&code, VERIFIER).await;
    assert_eq!(replay.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(replay).await,
        json!({ "detail": "Invalid or expired login code." })
    );
}

#[ntex::test]
async fn desktop_code_is_burned_by_a_wrong_verifier() {
    let h = Harness::new();
    let code = h.login_code();
    let wrong = h.exchange(&code, "wrong-verifier").await;
    assert_eq!(wrong.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        h.exchange(&code, VERIFIER).await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        h.exchange("not-a-code", VERIFIER).await.status(),
        StatusCode::BAD_REQUEST
    );
}

#[ntex::test]
async fn desktop_logout_ends_the_bearer_session() {
    let h = Harness::new();
    let logout = || test::TestRequest::post().uri("/accounts/desktop/logout/");
    assert_eq!(
        h.call(logout().to_request()).await.status(),
        StatusCode::UNAUTHORIZED
    );

    let res = h.call(h.bearer(logout(), &h.token)).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let me = test::TestRequest::get().uri("/api/me");
    assert_eq!(
        h.call(h.bearer(me, &h.token)).await.status(),
        StatusCode::UNAUTHORIZED
    );
}
