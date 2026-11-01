use ntex::web::{self, WebResponseError};
use serde_json::json;

pub type Result<T, E = AppError> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("unauthorized")]
    Unauthorized,
    #[error("cross-origin request rejected")]
    CrossOrigin,
    #[error("bad request: {0}")]
    BadRequest(&'static str),
    #[error("Discord account not approved")]
    NotApproved,
    #[error("Discord request failed: {0}")]
    Upstream(String),
}

impl AppError {
    pub fn to_response(&self) -> web::HttpResponse {
        match self {
            AppError::Db(e) => {
                eprintln!("{self}: {e:?}");
                web::HttpResponse::InternalServerError().into()
            }
            AppError::Unauthorized => {
                web::HttpResponse::Unauthorized().json(&json!({ "detail": "Unauthorized" }))
            }
            AppError::CrossOrigin => web::HttpResponse::Forbidden()
                .json(&json!({ "detail": "Cross-origin request rejected" })),
            AppError::BadRequest(detail) => {
                web::HttpResponse::BadRequest().json(&json!({ "detail": detail }))
            }
            AppError::NotApproved => {
                web::HttpResponse::Forbidden().body("Discord account not approved.")
            }
            AppError::Upstream(_) => {
                eprintln!("{self}");
                web::HttpResponse::BadGateway().body("Sign-in with Discord failed; try again.")
            }
        }
    }
}

impl<St, Err> WebResponseError<St, Err> for AppError {
    fn error_response(&self, _: &St) -> web::HttpResponse {
        self.to_response()
    }
}
