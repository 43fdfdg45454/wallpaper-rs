use crate::error::{Problem, Res};
use openidconnect::{AsyncHttpClient, HttpRequest, HttpResponse};
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::time::Duration;

pub struct Http(reqwest::Client);

impl<'c> AsyncHttpClient<'c> for Http {
    type Error = io::Error;
    type Future = Pin<Box<dyn Future<Output = Result<HttpResponse, io::Error>> + Send + 'c>>;

    fn call(&'c self, request: HttpRequest) -> Self::Future {
        Box::pin(async move {
            let request = reqwest::Request::try_from(request).map_err(io::Error::other)?;
            let response = self.0.execute(request).await.map_err(io::Error::other)?;
            let mut out = axum::http::Response::builder().status(response.status());
            for (name, value) in response.headers() {
                out = out.header(name, value);
            }
            out.body(response.bytes().await.map_err(io::Error::other)?.to_vec()).map_err(io::Error::other)
        })
    }
}

pub fn http() -> Res<Http> {
    let client = reqwest::Client::builder()
        .user_agent("wprs-server")
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10));
    Ok(Http(client.build().map_err(Problem::internal)?))
}
