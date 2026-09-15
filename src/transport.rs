//! HTTP transport abstraction.
//!
//! The [`Transport`] trait keeps the client testable: production uses
//! [`UreqTransport`], tests inject a mock and never touch the network.

use crate::error::{Error, Result};

/// A minimal HTTP request, independent of any HTTP library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// HTTP method, for example `GET` or `POST`.
    pub method: String,
    /// Fully qualified request URL.
    pub url: String,
    /// Optional request body (already form-encoded).
    pub body: Option<String>,
}

/// A minimal HTTP response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response body as UTF-8 text.
    pub body: String,
}

/// Executes HTTP requests on behalf of the client.
pub trait Transport {
    /// Executes `request`, authenticating with `authorization` (a full header
    /// value such as `Basic …`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Transport`] if no response could be obtained, or
    /// [`Error::Decode`] if the body could not be read as text.
    fn execute(&self, request: &HttpRequest, authorization: &str) -> Result<HttpResponse>;
}

/// Blocking HTTP transport backed by [`ureq`].
#[derive(Debug)]
pub struct UreqTransport {
    agent: ureq::Agent,
}

impl UreqTransport {
    /// Creates a transport with a default agent.
    #[must_use]
    pub fn new() -> Self {
        Self {
            agent: ureq::AgentBuilder::new().build(),
        }
    }
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for UreqTransport {
    fn execute(&self, request: &HttpRequest, authorization: &str) -> Result<HttpResponse> {
        let prepared = match request.method.as_str() {
            "GET" => self.agent.get(&request.url),
            "POST" => self.agent.post(&request.url),
            "DELETE" => self.agent.delete(&request.url),
            other => {
                return Err(Error::Transport(format!("unsupported method: {other}")));
            }
        }
        .set("Authorization", authorization)
        .set("Accept", "application/json");

        let result = match &request.body {
            Some(body) => prepared
                .set("Content-Type", "application/x-www-form-urlencoded")
                .send_string(body),
            None => prepared.call(),
        };

        match result {
            Ok(response) => read_response(response.status(), response),
            Err(ureq::Error::Status(status, response)) => read_response(status, response),
            Err(ureq::Error::Transport(error)) => Err(Error::Transport(error.to_string())),
        }
    }
}

fn read_response(status: u16, response: ureq::Response) -> Result<HttpResponse> {
    let body = response
        .into_string()
        .map_err(|error| Error::Decode(error.to_string()))?;
    Ok(HttpResponse { status, body })
}
