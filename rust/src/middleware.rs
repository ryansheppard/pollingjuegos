use ntex::http::{HeaderMap, Method, header};
use ntex::service::{Ctx, Middleware, Service};
use ntex::web::{WebRequest, WebResponse};

use crate::auth::{bearer_token, bearer_user, session_user};
use crate::error::AppError;
use crate::{AppData, St};

/// Rejects cross-origin cookie writes and requests without a valid session, and
/// stores the `AuthUser` in request extensions for the extractor.
pub struct RequireAuth;

impl<S> Middleware<S, St> for RequireAuth {
    type Service = RequireAuthService<S>;

    fn create(&self, st: &St, service: S) -> Self::Service {
        RequireAuthService {
            service,
            app: st.st().clone(),
        }
    }
}

pub struct RequireAuthService<S> {
    service: S,
    app: AppData,
}

impl<S, In> Service<St, WebRequest<In>> for RequireAuthService<S>
where
    S: Service<St, WebRequest<In>, Res = WebResponse>,
{
    type Res = WebResponse;
    type Error = S::Error;

    ntex::forward_bind!(St, service);
    ntex::forward_ready!(St, service);
    ntex::forward_shutdown!(St, service);

    async fn call(
        &self,
        req: WebRequest<In>,
        ctx: Ctx<'_, Self, St>,
    ) -> Result<WebResponse, S::Error> {
        // Browsers never attach an `Authorization` header on their own, so a
        // bearer request can't be forged cross-site and needs no origin check.
        let user = match bearer_token(req.headers()) {
            Some(token) => bearer_user(&self.app.db, token),
            None if !is_safe(req.method())
                && !same_origin(req.headers(), &self.app.cfg.public_origin) =>
            {
                return Ok(req.into_response(AppError::CrossOrigin.to_response()));
            }
            None => session_user(&self.app.db, &req),
        };
        let user = match user {
            Ok(Some(user)) => user,
            Ok(None) => return Ok(req.into_response(AppError::Unauthorized.to_response())),
            Err(e) => return Ok(req.into_response(AppError::Db(e).to_response())),
        };
        req.extensions_mut().insert(user);

        ctx.call(&self.service, req).await
    }
}

fn is_safe(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// Browsers always send `Origin` on cross-origin and non-GET requests;
/// `Sec-Fetch-Site` is the fallback for the rare same-origin write without it.
/// Non-browser clients can't carry a victim's cookie, so rejecting them costs nothing.
pub(crate) fn same_origin(headers: &HeaderMap, public_origin: &str) -> bool {
    match headers.get(header::ORIGIN) {
        Some(origin) => origin.as_bytes() == public_origin.as_bytes(),
        None => headers
            .get("sec-fetch-site")
            .is_some_and(|v| v.as_bytes() == b"same-origin"),
    }
}
