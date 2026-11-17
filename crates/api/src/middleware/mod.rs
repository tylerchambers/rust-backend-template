mod security;
mod transport;

use crate::state::State;
use axum::{Router, extract::DefaultBodyLimit, middleware};

/// Apply API policy only; bootstrap adds operational routes outside this boundary.
pub(crate) fn layers(router: Router<State>, state: State) -> Router {
    let router = router
        .layer(DefaultBodyLimit::max(16384))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            security::security,
        ))
        .with_state(state);
    transport::layers(router)
}

#[cfg(test)]
mod tests;
