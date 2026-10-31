use crate::{Error, Result};
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE};
use reqwest::redirect::Policy;
use reqwest::{Method, RequestBuilder, Response, Url};
use serde_json::Value;
use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use zeroize::Zeroizing;

pub const AGENT: &str = concat!("wallpaper-rs/", env!("CARGO_PKG_VERSION"));
pub const SMALL: usize = 1 << 20;
pub const PAGE: usize = 8 << 20;
pub const MAIL: usize = 2 << 20;

#[derive(Clone)]
pub struct Api {
    http: reqwest::Client,
    pub base: String,
    token: Option<Arc<Zeroizing<String>>>,
}

fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}

fn net(e: reqwest::Error) -> Error {
    match crate::pin::untrusted(&e) {
        true => Error::Untrusted,
        false => Error::Network(e.without_url().to_string()),
    }
}

pub fn check_url(url: &str) -> Result<String> {
    let parsed = Url::parse(url.trim()).map_err(|_| invalid("the server address is not a URL"))?;
    let host = parsed.host_str().unwrap_or_default();
    let local = host == "localhost"
        || host.trim_matches(['[', ']']).parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
    if parsed.scheme() != "https" && !(parsed.scheme() == "http" && local) {
        return Err(invalid("the server address must start with https://, or http:// for this computer"));
    }
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(invalid("the server address takes no user, query or fragment"));
    }
    Ok(parsed.as_str().trim_end_matches('/').to_owned())
}

impl Api {
    pub fn new(base: &str) -> Result<Api> {
        Api::pinned(base, None)
    }

    pub fn pinned(base: &str, pin: Option<&str>) -> Result<Api> {
        let builder = reqwest::Client::builder().user_agent(AGENT).referer(false).redirect(Policy::none());
        #[cfg(target_os = "android")]
        let builder = builder.tls_certs_only(
            webpki_root_certs::TLS_SERVER_ROOT_CERTS
                .iter()
                .filter_map(|der| reqwest::Certificate::from_der(der).ok()),
        );
        let builder = match pin.filter(|p| !p.is_empty()) {
            Some(pin) if crate::pin::valid_pin(pin) => {
                builder.tls_backend_preconfigured(crate::pin::config(Some(pin))?)
            }
            Some(_) => return Err(invalid("a certificate fingerprint has 64 lowercase hex digits")),
            None => builder,
        };
        let http = builder.connect_timeout(Duration::from_secs(10)).build().map_err(net)?;
        Ok(Api { http, base: check_url(base)?, token: None })
    }

    pub fn with_token(mut self, token: &str) -> Api {
        self.token = Some(Arc::new(Zeroizing::new(token.to_owned())));
        self
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let request = self.http.request(method, format!("{}{path}", self.base));
        match &self.token {
            Some(token) => request.bearer_auth(token.as_str()),
            None => request,
        }
    }

    async fn read(mut response: Response, max: usize) -> Result<Vec<u8>> {
        let large =
            || Error::Invalid(format!("the server sent an answer over {} KiB; nothing was used", max >> 10));
        if response.content_length().is_some_and(|n| n > max as u64) {
            return Err(large());
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(net)? {
            if body.len() + chunk.len() > max {
                return Err(large());
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }

    async fn answer(response: Response, max: usize) -> Result<Value> {
        let status = response.status();
        let body: Value = serde_json::from_slice(&Self::read(response, max).await?).unwrap_or(Value::Null);
        if status.is_success() {
            return Ok(body);
        }
        let code = body["code"].as_str().unwrap_or_default().to_owned();
        let detail =
            body["detail"].as_str().map_or_else(|| format!("the server answered {status}"), String::from);
        Err(Error::Server { status: status.as_u16(), code, detail })
    }

    pub async fn slow(&self, method: Method, path: &str) -> Result<Value> {
        let request = self.request(method, path).timeout(Duration::from_secs(600));
        Self::answer(request.send().await.map_err(net)?, SMALL).await
    }

    pub async fn call(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        let request = self.request(method, path).timeout(Duration::from_secs(30));
        let request = match body {
            Some(body) => request.json(&body),
            None => request,
        };
        Self::answer(request.send().await.map_err(net)?, SMALL).await
    }

    pub async fn get(&self, path: &str) -> Result<Value> {
        self.call(Method::GET, path, None).await
    }

    pub async fn fetch(&self, path: &str, max: usize) -> Result<Value> {
        let request = self.request(Method::GET, path).timeout(Duration::from_secs(60));
        Self::answer(request.send().await.map_err(net)?, max).await
    }

    pub async fn post(&self, path: &str, body: Value) -> Result<Value> {
        self.call(Method::POST, path, Some(body)).await
    }

    pub async fn put_file(&self, path: &str, file: &Path, size: u64) -> Result<Value> {
        let file = tokio::fs::File::open(file).await.map_err(|e| Error::Invalid(e.to_string()))?;
        let body = reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::new(file));
        let request = self.request(Method::PUT, path).header(CONTENT_LENGTH, size);
        let request = request.header(CONTENT_TYPE, "application/octet-stream").body(body);
        Self::answer(request.send().await.map_err(net)?, SMALL).await
    }

    pub async fn put_bytes(&self, path: &str, bytes: Vec<u8>) -> Result<Value> {
        let request = self.request(Method::PUT, path).timeout(Duration::from_secs(600));
        let request = request.header(CONTENT_TYPE, "application/octet-stream").body(bytes);
        Self::answer(request.send().await.map_err(net)?, SMALL).await
    }

    pub async fn get_bytes(&self, path: &str, max: usize) -> Result<Vec<u8>> {
        let request = self.request(Method::GET, path).timeout(Duration::from_secs(600));
        let response = request.send().await.map_err(net)?;
        match response.status().is_success() {
            true => Self::read(response, max).await,
            false => Err(Self::answer(response, SMALL).await.err().unwrap_or(invalid("no file data"))),
        }
    }

    pub async fn events(&self) -> Result<Response> {
        let response = self.request(Method::GET, "/api/v1/sync/events").send().await.map_err(net)?;
        match response.status().is_success() {
            true => Ok(response),
            false => Err(Self::answer(response, SMALL).await.err().unwrap_or(invalid("no event stream"))),
        }
    }
}
