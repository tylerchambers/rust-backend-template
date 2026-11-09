use axum::http::{HeaderMap, header};
use cookie::{Cookie, SameSite, time::Duration};

/// Preserve ambiguity until after parsing: a jar would discard duplicate names.
pub(crate) fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let mut value = None;
    for header in headers.get_all(header::COOKIE) {
        for parsed in Cookie::split_parse(header.to_str().ok()?) {
            let parsed = parsed.ok()?;
            if parsed.name() == name {
                if value.is_some() {
                    return None;
                }
                value = Some(parsed.value().to_owned());
            }
        }
    }
    value
}

pub(crate) fn session_cookie(token: String) -> Cookie<'static> {
    Cookie::build(("__Host-session", token))
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(Duration::days(7))
        .build()
}

pub(crate) fn remove_session_cookie() -> Cookie<'static> {
    let mut cookie = session_cookie(String::new());
    cookie.set_max_age(Duration::ZERO);
    cookie
}

pub(crate) fn csrf_cookie(token: String) -> Cookie<'static> {
    Cookie::build(("csrf", token))
        .secure(true)
        .same_site(SameSite::Strict)
        .path("/")
        .build()
}
