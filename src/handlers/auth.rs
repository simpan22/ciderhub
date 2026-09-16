use askama::Template;
use askama_web::WebTemplate;
use axum::extract::Request;
use axum::http::header::{HeaderMap, COOKIE, SET_COOKIE};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;

const COOKIE_NAME: &str = "ciderhub_auth";
// Intentionally a single hardcoded shared password, not a real account
// system — this just keeps casual visitors out, not a security boundary.
const PASSWORD: &str = "tage";
const ONE_YEAR_SECS: i64 = 60 * 60 * 24 * 365;

fn is_authenticated(headers: &HeaderMap) -> bool {
    let Some(cookie_header) = headers.get(COOKIE).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    cookie_header
        .split(';')
        .map(str::trim)
        .any(|pair| pair == format!("{COOKIE_NAME}={PASSWORD}"))
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

pub async fn require_auth(request: Request, next: Next) -> Response {
    let path = request.uri().path();
    if path == "/login" || path == "/healthz" || path.starts_with("/static/") {
        return next.run(request).await;
    }

    if is_authenticated(request.headers()) {
        return next.run(request).await;
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

pub async fn login(Form(form): Form<LoginForm>) -> Response {
    let next = safe_next(form.next);

    if form.password == PASSWORD {
        let mut response = Redirect::to(&next).into_response();
        let cookie = format!(
            "{COOKIE_NAME}={PASSWORD}; Path=/; HttpOnly; SameSite=Lax; Max-Age={ONE_YEAR_SECS}"
        );
        response
            .headers_mut()
            .insert(SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());
        response
    } else {
        (StatusCode::UNAUTHORIZED, LockedTemplate { next, error: true }).into_response()
    }
}
