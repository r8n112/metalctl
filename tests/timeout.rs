//! Verifies that `UreqTransport` actually enforces a request timeout.
//!
//! Uses a local `TcpListener` that accepts a connection and never responds, so
//! the test is offline (loopback only) and deterministic.

use std::io::Read;
use std::net::TcpListener;
use std::time::{Duration, Instant};

use metalctl::{api, Credentials, RetryPolicy, RobotClient, UreqTransport};

#[test]
fn times_out_when_the_server_never_responds() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    // Accept one connection, read the request, then hold the connection open
    // without ever writing a response.
    let _server = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = [0_u8; 1];
            let _ = stream.read(&mut buffer);
            std::thread::sleep(Duration::from_secs(1));
        }
    });

    let credentials = Credentials::new("user", "pass").unwrap();
    let transport = UreqTransport::with_timeout(Duration::from_millis(200));
    // Single attempt, so this measures the timeout, not the retry policy.
    let retry = RetryPolicy {
        max_attempts: 1,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(1),
    };
    let client = RobotClient::with_transport(format!("http://{addr}"), credentials, transport)
        .with_retry_policy(retry);

    let start = Instant::now();
    let result = api::server::list(&client);

    assert!(result.is_err(), "expected a timeout error");
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "timeout was not enforced (took {:?})",
        start.elapsed()
    );
}
