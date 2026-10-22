use axum::extract::{FromRequest, Request};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Form, Json};
use serde::de::DeserializeOwned;
use serde_json::json;
use std::fmt::Display;

#[derive(Debug)]
pub struct Problem {
    pub status: StatusCode,
    pub code: &'static str,
    pub detail: String,
    pub retry: Option<u64>,
}

pub type Res<T> = Result<T, Problem>;

impl Problem {
    pub fn new(status: StatusCode, code: &'static str, detail: impl Into<String>) -> Problem {
        Problem { status, code, detail: detail.into(), retry: None }
    }

    pub fn bad(detail: impl Into<String>) -> Problem {
        Problem::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid", detail)
    }

    pub fn unauthorized() -> Problem {
        Problem::new(StatusCode::UNAUTHORIZED, "unauthorized", "sign in or send a valid token")
    }

    pub fn forbidden(detail: impl Into<String>) -> Problem {
        Problem::new(StatusCode::FORBIDDEN, "forbidden", detail)
    }

    pub fn missing(what: &str) -> Problem {
        Problem::new(StatusCode::NOT_FOUND, "not_found", format!("{what} not found"))
    }

    pub fn conflict(code: &'static str, detail: impl Into<String>) -> Problem {
        Problem::new(StatusCode::CONFLICT, code, detail)
    }

    pub fn busy(seconds: u64) -> Problem {
        let detail = format!("too many requests, retry in {seconds} s");
        Problem {
            retry: Some(seconds),
            ..Problem::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", detail)
        }
    }

    pub fn internal(error: impl Display) -> Problem {
        tracing::error!(%error, "internal error");
        Problem::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", "internal error")
    }
}

pub fn require(ok: bool, detail: impl Into<String>) -> Res<()> {
    if ok { Ok(()) } else { Err(Problem::bad(detail)) }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let title = self.status.canonical_reason().unwrap_or("Error");
        let (status, code, detail) = (self.status.as_u16(), self.code, &self.detail);
        let body =
            json!({"type": "about:blank", "title": title, "status": status, "code": code, "detail": detail});
        let mut response =
            (self.status, [(header::CONTENT_TYPE, "application/problem+json")], body.to_string())
                .into_response();
        if let Some(seconds) = self.retry {
            response.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(seconds));
        }
        response
    }
}

macro_rules! internal {
    ($($error:ty),*) => {$(
        impl From<$error> for Problem {
            fn from(error: $error) -> Problem {
                Problem::internal(error)
            }
        }
    )*};
}

internal!(rusqlite::Error, std::io::Error, anyhow::Error, tokio::task::JoinError, serde_json::Error);

impl From<wprs_format::Error> for Problem {
    fn from(error: wprs_format::Error) -> Problem {
        match error {
            wprs_format::Error::Io(e) => Problem::internal(e),
            other => Problem::conflict("repository", other.to_string()),
        }
    }
}

pub struct Input<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for Input<T> {
    type Rejection = Problem;

    async fn from_request(req: Request, state: &S) -> Res<Input<T>> {
        let form = req
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|v| v.as_bytes().starts_with(b"application/x-www-form-urlencoded"));
        let parsed = if form {
            Form::<T>::from_request(req, state).await.map(|Form(v)| v).map_err(|e| e.body_text())
        } else {
            Json::<T>::from_request(req, state).await.map(|Json(v)| v).map_err(|e| e.body_text())
        };
        parsed.map(Input).map_err(|detail| Problem::new(StatusCode::BAD_REQUEST, "malformed", detail))
    }
}
