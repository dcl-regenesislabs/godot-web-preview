//! The subset of the reqwest 0.11 API that dclgodot uses, over the browser `fetch()`.
//!
//! Only for wasm32-unknown-emscripten without threads: every request is started and completed on
//! the browser main thread, and the response body arrives whole (`bytes_stream` yields one chunk).

use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use bytes::Bytes;
pub use http::{Method, StatusCode};
pub use url::Url;

pub mod header {
    pub use http::header::*;
}

pub mod redirect {
    #[derive(Clone, Copy, Debug)]
    pub struct Policy {
        pub(crate) follow: bool,
    }

    impl Policy {
        pub fn limited(_max: usize) -> Self {
            Self { follow: true }
        }
        pub fn none() -> Self {
            Self { follow: false }
        }
    }

    impl Default for Policy {
        fn default() -> Self {
            Self::limited(10)
        }
    }
}

mod fetch;

use header::{HeaderMap, HeaderName, HeaderValue};

#[derive(Debug)]
pub struct Error {
    message: String,
    timeout: bool,
    status: Option<StatusCode>,
}

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            timeout: false,
            status: None,
        }
    }
    pub fn is_timeout(&self) -> bool {
        self.timeout
    }
    pub fn is_status(&self) -> bool {
        self.status.is_some()
    }
    pub fn is_connect(&self) -> bool {
        !self.timeout && self.status.is_none()
    }
    pub fn status(&self) -> Option<StatusCode> {
        self.status
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub trait IntoUrl {
    fn into_url_string(self) -> String;
}

impl IntoUrl for &str {
    fn into_url_string(self) -> String {
        self.to_string()
    }
}
impl IntoUrl for String {
    fn into_url_string(self) -> String {
        self
    }
}
impl IntoUrl for &String {
    fn into_url_string(self) -> String {
        self.clone()
    }
}
impl IntoUrl for Url {
    fn into_url_string(self) -> String {
        self.into()
    }
}
impl IntoUrl for &Url {
    fn into_url_string(self) -> String {
        self.as_str().to_string()
    }
}

#[derive(Debug, Default, Clone)]
pub struct Body(Vec<u8>);

impl Body {
    pub fn as_bytes(&self) -> Option<&[u8]> {
        Some(&self.0)
    }
}

impl From<Vec<u8>> for Body {
    fn from(v: Vec<u8>) -> Self {
        Body(v)
    }
}
impl From<String> for Body {
    fn from(v: String) -> Self {
        Body(v.into_bytes())
    }
}
impl From<&'static str> for Body {
    fn from(v: &'static str) -> Self {
        Body(v.as_bytes().to_vec())
    }
}
impl From<&'static [u8]> for Body {
    fn from(v: &'static [u8]) -> Self {
        Body(v.to_vec())
    }
}
impl From<Bytes> for Body {
    fn from(v: Bytes) -> Self {
        Body(v.to_vec())
    }
}

#[derive(Debug, Clone, Default)]
struct ClientConfig {
    timeout: Option<Duration>,
    default_headers: HeaderMap,
    redirect: redirect::Policy,
}

#[derive(Debug, Clone, Default)]
pub struct Client {
    config: Arc<ClientConfig>,
}

impl Client {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }
    pub fn request<U: IntoUrl>(&self, method: Method, url: U) -> RequestBuilder {
        RequestBuilder {
            client: self.clone(),
            method,
            url: url.into_url_string(),
            headers: self.config.default_headers.clone(),
            body: None,
            timeout: self.config.timeout,
            error: None,
        }
    }
    pub fn get<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::GET, url)
    }
    pub fn post<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::POST, url)
    }
    pub fn put<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::PUT, url)
    }
    pub fn patch<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::PATCH, url)
    }
    pub fn delete<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::DELETE, url)
    }
    pub fn head<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::HEAD, url)
    }
}

#[derive(Debug, Default)]
pub struct ClientBuilder {
    config: ClientConfig,
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    /// Browsers own the User-Agent header; fetch() silently drops it.
    pub fn user_agent<V>(self, _value: V) -> Self {
        self
    }
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = Some(timeout);
        self
    }
    pub fn connect_timeout(self, _timeout: Duration) -> Self {
        self
    }
    pub fn pool_idle_timeout<D>(self, _timeout: D) -> Self {
        self
    }
    pub fn pool_max_idle_per_host(self, _max: usize) -> Self {
        self
    }
    pub fn redirect(mut self, policy: redirect::Policy) -> Self {
        self.config.redirect = policy;
        self
    }
    pub fn default_headers(mut self, headers: HeaderMap) -> Self {
        self.config.default_headers = headers;
        self
    }
    pub fn build(self) -> Result<Client> {
        Ok(Client {
            config: Arc::new(self.config),
        })
    }
}

#[derive(Debug)]
pub struct RequestBuilder {
    client: Client,
    method: Method,
    url: String,
    headers: HeaderMap,
    body: Option<Vec<u8>>,
    timeout: Option<Duration>,
    error: Option<Error>,
}

impl RequestBuilder {
    pub fn header<K, V>(mut self, key: K, value: V) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        match (HeaderName::try_from(key), HeaderValue::try_from(value)) {
            (Ok(k), Ok(v)) => {
                self.headers.append(k, v);
            }
            (Err(e), _) => self.error = Some(Error::new(e.into().to_string())),
            (_, Err(e)) => self.error = Some(Error::new(e.into().to_string())),
        }
        self
    }
    pub fn headers(mut self, headers: HeaderMap) -> Self {
        for (k, v) in headers.iter() {
            self.headers.insert(k.clone(), v.clone());
        }
        self
    }
    pub fn body<T: Into<Body>>(mut self, body: T) -> Self {
        self.body = Some(body.into().0);
        self
    }
    pub fn json<T: serde::Serialize + ?Sized>(mut self, json: &T) -> Self {
        match serde_json::to_vec(json) {
            Ok(v) => {
                if !self.headers.contains_key(header::CONTENT_TYPE) {
                    self.headers.insert(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("application/json"),
                    );
                }
                self.body = Some(v);
            }
            Err(e) => self.error = Some(Error::new(e.to_string())),
        }
        self
    }
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
    pub fn bearer_auth<T: fmt::Display>(self, token: T) -> Self {
        self.header(header::AUTHORIZATION, format!("Bearer {token}"))
    }

    pub async fn send(self) -> Result<Response> {
        if let Some(e) = self.error {
            return Err(e);
        }
        let headers: Vec<(String, String)> = self
            .headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or_default().to_string()))
            .collect();
        let raw = fetch::fetch(fetch::Request {
            method: self.method.as_str().to_string(),
            url: self.url.clone(),
            headers_json: serde_json::to_string(&headers).unwrap_or_else(|_| "[]".into()),
            body: self.body.unwrap_or_default(),
            timeout_ms: self.timeout.map(|t| t.as_millis() as i32).unwrap_or(0),
            follow_redirects: self.client.config.redirect.follow,
        })
        .await;

        match raw.status {
            -1 => Err(Error {
                message: format!("request timed out: {}", self.url),
                timeout: true,
                status: None,
            }),
            0 => Err(Error::new(format!(
                "fetch failed ({}): {}",
                self.url, raw.meta
            ))),
            status => {
                let (final_url, header_pairs): (String, Vec<(String, String)>) =
                    serde_json::from_str(&raw.meta).unwrap_or_else(|_| (self.url.clone(), vec![]));
                let mut headers = HeaderMap::new();
                for (k, v) in header_pairs {
                    if let (Ok(k), Ok(v)) = (HeaderName::try_from(k), HeaderValue::try_from(v)) {
                        headers.append(k, v);
                    }
                }
                Ok(Response {
                    status: StatusCode::from_u16(status as u16).unwrap_or(StatusCode::OK),
                    headers,
                    url: Url::parse(&final_url).ok(),
                    body: Bytes::from(raw.body),
                })
            }
        }
    }
}

#[derive(Debug)]
pub struct Response {
    status: StatusCode,
    headers: HeaderMap,
    url: Option<Url>,
    body: Bytes,
}

impl Response {
    pub fn status(&self) -> StatusCode {
        self.status
    }
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }
    pub fn url(&self) -> Option<&Url> {
        self.url.as_ref()
    }
    pub fn content_length(&self) -> Option<u64> {
        Some(self.body.len() as u64)
    }
    pub fn error_for_status(self) -> Result<Self> {
        if self.status.is_client_error() || self.status.is_server_error() {
            Err(Error {
                message: format!("HTTP status {}", self.status),
                timeout: false,
                status: Some(self.status),
            })
        } else {
            Ok(self)
        }
    }
    pub async fn bytes(self) -> Result<Bytes> {
        Ok(self.body)
    }
    pub async fn text(self) -> Result<String> {
        Ok(String::from_utf8_lossy(&self.body).into_owned())
    }
    pub async fn json<T: serde::de::DeserializeOwned>(self) -> Result<T> {
        serde_json::from_slice(&self.body).map_err(|e| Error::new(e.to_string()))
    }
    pub fn bytes_stream(self) -> impl futures_util::Stream<Item = Result<Bytes>> {
        futures_util::stream::once(std::future::ready(Ok(self.body)))
    }
}

/// Completion slot shared between a pending request future and the JS callback.
#[derive(Default)]
struct Slot {
    done: Option<fetch::RawResponse>,
    waker: Option<std::task::Waker>,
}

type SlotRef = Arc<Mutex<Slot>>;
