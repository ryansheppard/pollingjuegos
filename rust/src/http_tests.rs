//! Drives the real route table so middleware, extractors and JSON shapes are
//! exercised together; the per-module tests only cover the SQL layer.

use std::sync::Arc;

use cookie::Cookie;
use ntex::http::{Request, Response, StatusCode, header};
use ntex::web::{self, test};
use serde_json::{Value, json};

use crate::auth::{self, SESSION_COOKIE};
use crate::config::Config;
use crate::db::{self, Db};
use crate::{AppData, St};

const ORIGIN: &str = "https://pj.example";

struct Harness {
    db: Db,
    user_id: i64,
    token: String,
}

impl Harness {
    fn new() -> Self {
        let db = Db::new(db::test_conn());
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
        Self { db, user_id, token }
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
            nfl: Default::default(),
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
    let oauth_value = set_cookie(&login, "__Host-oauth")
        .and_then(|c| c.split(';').next())
        .and_then(|kv| kv.split_once('='))
        .map(|(_, v)| v.to_owned())
        .unwrap();
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
