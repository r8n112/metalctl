//! The Robot API client.

use serde::de::DeserializeOwned;

use crate::credentials::Credentials;
use crate::error::{Error, Result};
use crate::transport::{HttpRequest, HttpResponse, Transport, UreqTransport};

/// Default Robot API base URL.
pub const DEFAULT_BASE_URL: &str = "https://robot-ws.your-server.de";

/// Typed client for the Hetzner Robot API.
///
/// The generic parameter is the [`Transport`] used to perform requests, which
/// allows tests to run without network access.
pub struct RobotClient<T = UreqTransport> {
    base_url: String,
    credentials: Credentials,
    transport: T,
}

impl RobotClient<UreqTransport> {
    /// Creates a client against [`DEFAULT_BASE_URL`] using the default transport.
    #[must_use]
    pub fn new(credentials: Credentials) -> Self {
        Self::with_transport(DEFAULT_BASE_URL, credentials, UreqTransport::new())
    }
}

impl<T: Transport> RobotClient<T> {
    /// Creates a client with an explicit base URL and transport.
    #[must_use]
    pub fn with_transport(
        base_url: impl Into<String>,
        credentials: Credentials,
        transport: T,
    ) -> Self {
        let mut base_url = base_url.into();
        while base_url.ends_with('/') {
            base_url.pop();
        }
        Self {
            base_url,
            credentials,
            transport,
        }
    }

    /// Returns the configured base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Performs a `GET` request and deserialises the JSON body.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Api`] for a non-success status, [`Error::Decode`] if the
    /// body is not valid JSON, or [`Error::Transport`] if the request fails.
    pub fn get_json<V: DeserializeOwned>(&self, path: &str) -> Result<V> {
        let response = self.send("GET", path, None)?;
        deserialize(&response.body)
    }

    /// Performs a form-encoded `POST` request and deserialises the JSON body.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Api`] for a non-success status, [`Error::Decode`] if the
    /// body is not valid JSON, or [`Error::Transport`] if the request fails.
    pub fn post_form<V: DeserializeOwned>(&self, path: &str, form: &[(&str, &str)]) -> Result<V> {
        let body = encode_form(form);
        let response = self.send("POST", path, Some(body))?;
        deserialize(&response.body)
    }

    /// Performs a form-encoded `POST` request and discards the response body.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Api`] for a non-success status or [`Error::Transport`]
    /// if the request fails.
    pub fn post_form_ok(&self, path: &str, form: &[(&str, &str)]) -> Result<()> {
        let body = encode_form(form);
        self.send("POST", path, Some(body))?;
        Ok(())
    }

    /// Performs a `DELETE` request and discards the response body.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Api`] for a non-success status or [`Error::Transport`]
    /// if the request fails.
    pub fn delete(&self, path: &str) -> Result<()> {
        self.send("DELETE", path, None)?;
        Ok(())
    }

    /// Performs a form-encoded `DELETE` request and discards the response body.
    ///
    /// Some Robot endpoints (for example vSwitch disconnects) require a form
    /// body on `DELETE`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Api`] for a non-success status or [`Error::Transport`]
    /// if the request fails.
    pub fn delete_form(&self, path: &str, form: &[(&str, &str)]) -> Result<()> {
        let body = encode_form(form);
        self.send("DELETE", path, Some(body))?;
        Ok(())
    }

    fn send(&self, method: &str, path: &str, body: Option<String>) -> Result<HttpResponse> {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));
        let request = HttpRequest {
            method: method.to_owned(),
            url,
            body,
        };
        let response = self
            .transport
            .execute(&request, &self.credentials.authorization())?;
        if !(200..300).contains(&response.status) {
            return Err(Error::Api {
                status: response.status,
                message: extract_error_message(&response.body),
            });
        }
        Ok(response)
    }
}

fn deserialize<V: DeserializeOwned>(body: &str) -> Result<V> {
    serde_json::from_str(body).map_err(|error| Error::Decode(error.to_string()))
}

fn encode_form(form: &[(&str, &str)]) -> String {
    form.iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn percent_encode(input: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            // Brackets are kept literal so PHP-style array parameters such as
            // `server[]` and `ip[]` reach the API as documented.
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'[' | b']' => {
                encoded.push(char::from(byte));
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push('%');
                encoded.push(char::from(HEX[usize::from(byte >> 4)]));
                encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
        }
    }
    encoded
}

fn extract_error_message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| body.trim().chars().take(200).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_form_parameters() {
        let encoded = encode_form(&[("a b", "c&d"), ("x", "1")]);
        assert_eq!(encoded, "a+b=c%26d&x=1");
    }

    #[test]
    fn keeps_brackets_literal_in_form_keys() {
        let encoded = encode_form(&[("server[]", "1"), ("server[]", "2")]);
        assert_eq!(encoded, "server[]=1&server[]=2");
    }

    #[test]
    fn extracts_api_error_message() {
        let body = r#"{"error":{"status":401,"code":"UNAUTHORIZED","message":"unauthorized"}}"#;
        assert_eq!(extract_error_message(body), "unauthorized");
    }

    #[test]
    fn falls_back_to_body_snippet() {
        assert_eq!(extract_error_message("  not json  "), "not json");
    }
}
