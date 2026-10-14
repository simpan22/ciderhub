use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Request, State};
use axum::http::header::{HeaderMap, COOKIE, SET_COOKIE};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;

use crate::state::AppState;

const COOKIE_NAME: &str = "ciderhub_auth";
pub const TASTER_COOKIE_NAME: &str = "ciderhub_taster";
pub const ONE_YEAR_SECS: i64 = 60 * 60 * 24 * 365;

/// Who is using a `/taste` page, added to the request extensions by
/// `require_auth`: `None` is the owner (password cookie), `Some` a
/// friend signed in through their invite link. See
/// docs/specs/friend-tasting-page.md.
#[derive(Clone)]
pub struct CurrentTaster(pub Option<Taster>);

#[derive(Clone)]
pub struct Taster {
    pub id: i64,
    pub name: String,
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())?
        .split(';')
        .map(str::trim)
        .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
}

/// The only paths a taster cookie opens. `/tasters` (the owner's page
/// for managing invite links) deliberately doesn't match.
pub fn is_taste_path(path: &str) -> bool {
    path == "/taste" || path.starts_with("/taste/")
}

/// A non-revoked taster with this invite token, if any.
pub async fn find_taster(db: &sqlx::SqlitePool, token: &str) -> Result<Option<Taster>, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT id as "id!", name FROM tasters WHERE token = ? AND revoked_at IS NULL"#,
        token
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| Taster { id: r.id, name: r.name }))
}

// A single shared password (from APP_PASSWORD, see config.rs), not a
// real account system — this just keeps casual visitors out, not a
// security boundary.
fn is_authenticated(headers: &HeaderMap, app_password: &str) -> bool {
    let Some(cookie_header) = headers.get(COOKIE).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    cookie_header
        .split(';')
        .map(str::trim)
        .any(|pair| pair == format!("{COOKIE_NAME}={app_password}"))
}

/// Only `/` is a safe redirect target we trust from user input — anything
/// not starting with a single leading slash could be an off-site redirect.
fn safe_next(next: Option<String>) -> String {
    match next {
        Some(n) if n.starts_with('/') && !n.starts_with("//") => n,
        _ => "/".to_string(),
    }
}

#[derive(Template, WebTemplate)]
#[template(path = "locked.html")]
pub struct LockedTemplate {
    pub next: String,
    pub error: bool,
}

#[derive(Template, WebTemplate)]
#[template(path = "taste/invalid_link.html")]
pub struct InvalidLinkTemplate;

pub async fn require_auth(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    // `/taste/join/{token}` checks the token itself — it's how a taster
    // gets their cookie in the first place.
    if path == "/login" || path == "/healthz" || path.starts_with("/static/") || path.starts_with("/taste/join/") {
        return next.run(request).await;
    }

    if is_authenticated(request.headers(), &state.app_password) {
        request.extensions_mut().insert(CurrentTaster(None));
        return next.run(request).await;
    }

    if is_taste_path(&path) {
        if let Some(token) = cookie_value(request.headers(), TASTER_COOKIE_NAME) {
            let taster = match find_taster(&state.db, token).await {
                Ok(taster) => taster,
                Err(err) => {
                    tracing::error!("taster lookup failed: {err:#}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
                }
            };
            return match taster {
                Some(taster) => {
                    request.extensions_mut().insert(CurrentTaster(Some(taster)));
                    next.run(request).await
                }
                // A revoked (or replaced) link: say so rather than
                // showing the owner's password prompt. htmx doesn't swap
                // 4xx responses, so a save from an open page reloads it
                // instead, which then lands here as a full page.
                None if request.headers().contains_key("HX-Request") => {
                    let mut response = StatusCode::OK.into_response();
                    response.headers_mut().insert("HX-Refresh", HeaderValue::from_static("true"));
                    response
                }
                None => (StatusCode::UNAUTHORIZED, InvalidLinkTemplate).into_response(),
            };
        }
    }

    let next_path = request
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());

    (
        StatusCode::UNAUTHORIZED,
        LockedTemplate {
            next: next_path,
            error: false,
        },
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct LoginForm {
    password: String,
    #[serde(default)]
    next: Option<String>,
}

pub async fn login(State(state): State<AppState>, Form(form): Form<LoginForm>) -> Response {
    let next = safe_next(form.next);

    if form.password == state.app_password {
        let mut response = Redirect::to(&next).into_response();
        let cookie = format!(
            "{COOKIE_NAME}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={ONE_YEAR_SECS}",
            state.app_password
        );
        response
            .headers_mut()
            .insert(SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());
        response
    } else {
        (StatusCode::UNAUTHORIZED, LockedTemplate { next, error: true }).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taster_cookie_only_opens_taste_paths() {
        assert!(is_taste_path("/taste"));
        assert!(is_taste_path("/taste/comparisons"));
        assert!(is_taste_path("/taste/comparisons/12"));
        assert!(!is_taste_path("/tasters"));
        assert!(!is_taste_path("/tasters/1/revoke"));
        assert!(!is_taste_path("/tastefully"));
        assert!(!is_taste_path("/batches"));
        assert!(!is_taste_path("/"));
    }

    #[test]
    fn reads_one_cookie_among_several() {
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, HeaderValue::from_static("a=1; ciderhub_taster=abc; ciderhub_tasterx=no"));
        assert_eq!(cookie_value(&headers, TASTER_COOKIE_NAME), Some("abc"));
        assert_eq!(cookie_value(&headers, "missing"), None);
        // A cookie whose name merely starts with ours doesn't match.
        headers.insert(COOKIE, HeaderValue::from_static("ciderhub_tasterx=no"));
        assert_eq!(cookie_value(&headers, TASTER_COOKIE_NAME), None);
    }
}
