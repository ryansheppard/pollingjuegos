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
    #[error("not found")]
    NotFound,
    #[error("bad request: {0}")]
    BadRequest(&'static str),
    #[error("Discord account not approved")]
    NotApproved,
    #[error("Discord request failed: {0}")]
    Upstream(String),
    #[error("serializing response: {0}")]
    Serialize(#[from] serde_json::Error),
}

impl AppError {
    pub fn to_response(&self) -> web::HttpResponse {
        match self {
            AppError::Db(e) => {
                eprintln!("{self}: {e:?}");
                web::HttpResponse::InternalServerError().into()
            }
            AppError::Unauthorized => crate::json(
                web::HttpResponse::Unauthorized(),
                &json!({ "detail": "Unauthorized" }),
            ),
            AppError::CrossOrigin => crate::json(
                web::HttpResponse::Forbidden(),
                &json!({ "detail": "Cross-origin request rejected" }),
            ),
            AppError::NotFound => crate::json(
                web::HttpResponse::NotFound(),
                &json!({ "detail": "Not Found" }),
            ),
            AppError::BadRequest(detail) => crate::json(
                web::HttpResponse::BadRequest(),
                &json!({ "detail": detail }),
            ),
            AppError::NotApproved => {
                web::HttpResponse::Forbidden().body("Discord account not approved.")
            }
            AppError::Serialize(_) => {
                eprintln!("{self}");
                web::HttpResponse::InternalServerError().into()
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
