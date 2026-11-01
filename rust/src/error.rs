use ntex::web::{self, WebResponseError};

pub type Result<T, E = AppError> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
}

impl<St, Err> WebResponseError<St, Err> for AppError {
    fn error_response(&self, _: &St) -> web::HttpResponse {
        match self {
            AppError::Db(e) => {
                eprintln!("{self}: {e:?}");
                web::HttpResponse::InternalServerError().into()
            }
        }
    }
}
