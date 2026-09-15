//! Integration tests for the Robot client using an in-memory transport.
//!
//! These tests never touch the network: they assert both the parsed result and
//! the exact HTTP request the client produced.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use metalctl::api;
use metalctl::{Credentials, Error, HttpRequest, HttpResponse, RobotClient, Transport};

#[derive(Clone, Default)]
struct MockTransport {
    state: Rc<RefCell<State>>,
}

#[derive(Default)]
struct State {
    responses: VecDeque<HttpResponse>,
    requests: Vec<(HttpRequest, String)>,
}

impl MockTransport {
    fn with_responses(responses: Vec<HttpResponse>) -> Self {
        let transport = Self::default();
        transport.state.borrow_mut().responses = responses.into();
        transport
    }

    fn requests(&self) -> Vec<(HttpRequest, String)> {
        self.state.borrow().requests.clone()
    }
}

impl Transport for MockTransport {
    fn execute(
        &self,
        request: &HttpRequest,
        authorization: &str,
    ) -> metalctl::Result<HttpResponse> {
        let mut state = self.state.borrow_mut();
        state
            .requests
            .push((request.clone(), authorization.to_owned()));
        state
            .responses
            .pop_front()
            .ok_or_else(|| Error::Transport("no mock response queued".to_owned()))
    }
}

fn client_with(responses: Vec<HttpResponse>) -> (RobotClient<MockTransport>, MockTransport) {
    let transport = MockTransport::with_responses(responses);
    let credentials = Credentials::new("user", "pass").unwrap();
    let client =
        RobotClient::with_transport("https://robot.example", credentials, transport.clone());
    (client, transport)
}

#[test]
fn lists_servers_with_basic_auth() {
    let body = r#"[{"server":{"server_number":321,"server_name":"alpha","server_ip":"192.0.2.1"}},
                   {"server":{"server_number":322,"server_name":"beta","server_ip":"192.0.2.2","product":"AX41"}}]"#;
    let (client, transport) = client_with(vec![HttpResponse {
        status: 200,
        body: body.to_owned(),
    }]);

    let servers = api::server::list(&client).unwrap();
    assert_eq!(servers.len(), 2);
    assert_eq!(servers[0].server_number, 321);
    assert_eq!(servers[0].server_name, "alpha");
    assert_eq!(servers[1].product.as_deref(), Some("AX41"));

    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0.method, "GET");
    assert_eq!(requests[0].0.url, "https://robot.example/server");
    assert_eq!(requests[0].0.body, None);
    assert_eq!(requests[0].1, "Basic dXNlcjpwYXNz");
}

#[test]
fn gets_a_single_server() {
    let body = r#"{"server":{"server_number":321,"server_name":"alpha","server_ip":"192.0.2.1","server_ipv6_net":"2a01:db8::/64"}}"#;
    let (client, transport) = client_with(vec![HttpResponse {
        status: 200,
        body: body.to_owned(),
    }]);

    let server = api::server::get(&client, 321).unwrap();
    assert_eq!(server.server_ipv6_net.as_deref(), Some("2a01:db8::/64"));
    assert_eq!(
        transport.requests()[0].0.url,
        "https://robot.example/server/321"
    );
}

#[test]
fn maps_api_errors() {
    let body = r#"{"error":{"status":401,"code":"UNAUTHORIZED","message":"unauthorized"}}"#;
    let (client, _transport) = client_with(vec![HttpResponse {
        status: 401,
        body: body.to_owned(),
    }]);

    match api::server::list(&client).unwrap_err() {
        Error::Api { status, message } => {
            assert_eq!(status, 401);
            assert_eq!(message, "unauthorized");
        }
        other => panic!("expected Error::Api, got {other}"),
    }
}

#[test]
fn normalises_a_trailing_slash_in_the_base_url() {
    let transport = MockTransport::with_responses(vec![HttpResponse {
        status: 200,
        body: "[]".to_owned(),
    }]);
    let credentials = Credentials::new("u", "p").unwrap();
    let client =
        RobotClient::with_transport("https://robot.example/", credentials, transport.clone());

    assert_eq!(client.base_url(), "https://robot.example");
    assert!(api::server::list(&client).unwrap().is_empty());
    assert_eq!(
        transport.requests()[0].0.url,
        "https://robot.example/server"
    );
}

#[test]
fn gets_reverse_dns() {
    let body = r#"{"rdns":{"ip":"192.0.2.1","ptr":"host.example.com"}}"#;
    let (client, transport) = client_with(vec![HttpResponse {
        status: 200,
        body: body.to_owned(),
    }]);

    let entry = api::rdns::get(&client, "192.0.2.1").unwrap();
    assert_eq!(entry.ip, "192.0.2.1");
    assert_eq!(entry.ptr.as_deref(), Some("host.example.com"));
    assert_eq!(
        transport.requests()[0].0.url,
        "https://robot.example/rdns/192.0.2.1"
    );
}

#[test]
fn sets_reverse_dns_with_form_body() {
    let body = r#"{"rdns":{"ip":"192.0.2.1","ptr":"new.example.com"}}"#;
    let (client, transport) = client_with(vec![HttpResponse {
        status: 200,
        body: body.to_owned(),
    }]);

    let entry = api::rdns::set(&client, "192.0.2.1", "new.example.com").unwrap();
    assert_eq!(entry.ptr.as_deref(), Some("new.example.com"));

    let request = &transport.requests()[0].0;
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://robot.example/rdns/192.0.2.1");
    assert_eq!(request.body.as_deref(), Some("ptr=new.example.com"));
}

#[test]
fn lists_reset_methods() {
    let body = r#"{"reset":{"server_number":321,"type":["sw","hw","power"]}}"#;
    let (client, transport) = client_with(vec![HttpResponse {
        status: 200,
        body: body.to_owned(),
    }]);

    let options = api::reset::options(&client, 321).unwrap();
    assert_eq!(options.server_number, 321);
    assert_eq!(options.types, vec!["sw", "hw", "power"]);
    assert_eq!(
        transport.requests()[0].0.url,
        "https://robot.example/reset/321"
    );
}

#[test]
fn executes_a_reset() {
    let body = r#"{"reset":{"server_number":321,"type":"hw"}}"#;
    let (client, transport) = client_with(vec![HttpResponse {
        status: 200,
        body: body.to_owned(),
    }]);

    let result = api::reset::execute(&client, 321, api::reset::ResetType::Hardware).unwrap();
    assert_eq!(result.kind, "hw");

    let request = &transport.requests()[0].0;
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://robot.example/reset/321");
    assert_eq!(request.body.as_deref(), Some("type=hw"));
}

#[test]
fn gets_rescue_configuration() {
    let body =
        r#"{"rescue":{"server_number":321,"os":"linux","arch":64,"active":true,"password":"pw"}}"#;
    let (client, transport) = client_with(vec![HttpResponse {
        status: 200,
        body: body.to_owned(),
    }]);

    let rescue = api::boot::rescue(&client, 321).unwrap();
    assert_eq!(rescue.os.as_deref(), Some("linux"));
    assert_eq!(rescue.arch, Some(64));
    assert_eq!(rescue.active, Some(true));
    assert_eq!(
        transport.requests()[0].0.url,
        "https://robot.example/boot/321/rescue"
    );
}

#[test]
fn activates_rescue_with_authorized_keys() {
    let body = r#"{"rescue":{"server_number":321,"os":"linux","arch":64,"active":true}}"#;
    let (client, transport) = client_with(vec![HttpResponse {
        status: 200,
        body: body.to_owned(),
    }]);

    let rescue = api::boot::activate_rescue(
        &client,
        321,
        "linux",
        "64",
        &["ssh-ed25519 AAAA".to_owned()],
    )
    .unwrap();
    assert_eq!(rescue.active, Some(true));

    let request = &transport.requests()[0].0;
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://robot.example/boot/321/rescue");
    assert_eq!(
        request.body.as_deref(),
        Some("os=linux&arch=64&authorized_key=ssh-ed25519+AAAA")
    );
}

#[test]
fn deactivates_rescue() {
    let (client, transport) = client_with(vec![HttpResponse {
        status: 200,
        body: r#"{"rescue":{"server_number":321}}"#.to_owned(),
    }]);

    api::boot::deactivate_rescue(&client, 321).unwrap();
    let request = &transport.requests()[0].0;
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.url, "https://robot.example/boot/321/rescue");
}
