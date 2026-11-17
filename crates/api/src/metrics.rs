use crate::{error::ApiError, state::State};
use axum::{
    extract::State as Extract,
    http::header,
    response::{IntoResponse, Response},
};
use domain::Error;
use prometheus_client::encoding::text::encode;
use prometheus_client::{
    metrics::{counter::Counter, histogram::Histogram},
    registry::Registry,
};

pub(crate) struct HttpMetrics {
    pub requests: Counter,
    pub duration: Histogram,
    pub rate_limited: Counter,
    pub authentication_denials: Counter,
    pub authorization_denials: Counter,
}
impl Default for HttpMetrics {
    fn default() -> Self {
        Self {
            requests: Counter::default(),
            duration: Histogram::new([
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 15.0,
            ]),
            rate_limited: Counter::default(),
            authentication_denials: Counter::default(),
            authorization_denials: Counter::default(),
        }
    }
}

impl HttpMetrics {
    pub fn register(&self, registry: &mut Registry) {
        registry.register(
            "http_requests",
            "Completed HTTP requests",
            self.requests.clone(),
        );
        registry.register(
            "http_request_duration_seconds",
            "HTTP request latency",
            self.duration.clone(),
        );
        registry.register(
            "authentication_rate_limited",
            "HTTP responses rejected by admission budgets",
            self.rate_limited.clone(),
        );
        registry.register(
            "authentication_denials",
            "HTTP unauthorized responses",
            self.authentication_denials.clone(),
        );
        registry.register(
            "authorization_denials",
            "HTTP forbidden responses including CSRF rejection",
            self.authorization_denials.clone(),
        );
    }
}

pub(crate) async fn http_metrics(Extract(state): Extract<State>) -> Result<Response, ApiError> {
    let mut registry = Registry::default();
    state.register_metrics(&mut registry);
    let mut output = String::new();
    encode(&mut output, &registry).map_err(|_| Error::Unavailable)?;
    Ok((
        [(
            header::CONTENT_TYPE,
            "application/openmetrics-text; version=1.0.0; charset=utf-8",
        )],
        output,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    #[test]
    fn registries_are_isolated_and_histograms_count_each_observation() {
        let first = super::HttpMetrics::default();
        let second = super::HttpMetrics::default();
        first.requests.inc();
        first.duration.observe(0.02);
        first.duration.observe(0.5);
        assert_eq!(first.requests.get(), 1);
        assert_eq!(second.requests.get(), 0);
        let mut registry = prometheus_client::registry::Registry::default();
        first.register(&mut registry);
        let mut encoded = String::new();
        assert!(prometheus_client::encoding::text::encode(&mut encoded, &registry).is_ok());
        assert!(encoded.contains("http_request_duration_seconds_count 2"));
        assert!(encoded.contains("http_request_duration_seconds_sum 0.52"));
    }
}
