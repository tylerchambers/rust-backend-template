use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::Error;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct ErrorBody {
    pub(crate) code: String,
}
pub(crate) struct ApiError(pub(crate) Error);
impl From<Error> for ApiError {
    fn from(value: Error) -> Self {
        Self(value)
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0 {
            Error::Invalid => StatusCode::BAD_REQUEST,
            Error::Unauthenticated => StatusCode::UNAUTHORIZED,
            Error::Forbidden => StatusCode::FORBIDDEN,
            Error::NotFound => StatusCode::NOT_FOUND,
            Error::Conflict => StatusCode::CONFLICT,
            Error::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Error::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        };
        (
            status,
            Json(ErrorBody {
                code: self.0.to_string(),
            }),
        )
            .into_response()
    }
}
