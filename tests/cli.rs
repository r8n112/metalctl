//! End-to-end CLI tests against an in-process HTTP stub.
//!
//! The compiled `metalctl` binary is pointed at a loopback stub via the hidden
//! `--base-url` flag, so these tests exercise argument parsing, the credential
//! resolution order, the destructive-operation safety contract, and the exact
//! HTTP request — all without network egress.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone, Debug)]
struct Recorded {
    method: String,
    path: String,
    body: String,
    authorization: Option<String>,
}

struct Stub {
    base_url: String,
    requests: Arc<Mutex<Vec<Recorded>>>,
    _thread: thread::JoinHandle<()>,
}

impl Stub {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub");
        let addr = listener.local_addr().expect("stub address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        let handle = thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => handle_connection(&stream, &recorded),
                    Err(_) => break,
                }
            }
        });
        Self {
            base_url: format!("http://{addr}"),
            requests,
            _thread: handle,
        }
    }

    fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().expect("stub lock").clone()
    }
}

fn handle_connection(stream: &TcpStream, recorded: &Arc<Mutex<Vec<Recorded>>>) {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();

    let mut content_length = 0_usize;
    let mut authorization = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            match key.trim().to_ascii_lowercase().as_str() {
                "content-length" => content_length = value.trim().parse().unwrap_or(0),
                "authorization" => authorization = Some(value.trim().to_string()),
                _ => {}
            }
        }
    }

    let mut body = String::new();
    if content_length > 0 {
        let mut buffer = vec![0_u8; content_length];
        let _ = reader.read_exact(&mut buffer);
        body = String::from_utf8_lossy(&buffer).into_owned();
    }

    recorded.lock().expect("stub lock").push(Recorded {
        method,
        path: path.clone(),
        body,
        authorization,
    });

    let response_body = if path.ends_with("/server") {
        r#"[{"server":{"server_number":321,"server_name":"alpha","server_ip":"192.0.2.1"}}]"#
    } else {
        "{}"
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response_body.len(),
        response_body
    );
    let mut writer = stream;
    let _ = writer.write_all(response.as_bytes());
    let _ = writer.flush();
}

fn run_cli(stub: &Stub, credentials: bool, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_metalctl"));
    command
        .arg("--base-url")
        .arg(&stub.base_url)
        .args(args)
        .env("METALCTL_CONFIG", "/nonexistent/metalctl-e2e-config")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if credentials {
        command
            .env("HETZNER_ROBOT_USER", "user")
            .env("HETZNER_ROBOT_PASSWORD", "pass");
    } else {
        command
            .env_remove("HETZNER_ROBOT_USER")
            .env_remove("HETZNER_ROBOT_PASSWORD");
    }
    command.output().expect("run metalctl")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn dry_run_prints_and_sends_no_request() {
    let stub = Stub::start();
    let output = run_cli(&stub, true, &["vswitch", "cancel", "5", "--dry-run"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(
        stub.requests().is_empty(),
        "dry-run must not touch the network"
    );
    assert!(stderr(&output).contains("dry-run"));
}

#[test]
fn dry_run_succeeds_without_credentials() {
    let stub = Stub::start();
    // No HETZNER_ROBOT_* in the environment and no flags: a dry-run must still
    // succeed and must not build a usable authenticated client.
    let output = run_cli(&stub, false, &["vswitch", "cancel", "5", "--dry-run"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(
        stub.requests().is_empty(),
        "dry-run must not touch the network"
    );
    assert!(stderr(&output).contains("dry-run"));
}

#[test]
fn refuses_without_confirmation_in_a_non_interactive_session() {
    let stub = Stub::start();
    // stdin is null, so this is non-interactive; without --yes it must refuse.
    let output = run_cli(&stub, true, &["vswitch", "cancel", "5"]);

    assert!(!output.status.success());
    assert!(
        stub.requests().is_empty(),
        "refusal must not touch the network"
    );
    assert!(stderr(&output).contains("confirmation"));
}

#[test]
fn yes_executes_and_sends_the_expected_request() {
    let stub = Stub::start();
    let output = run_cli(&stub, true, &["vswitch", "cancel", "5", "--yes"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let requests = stub.requests();
    assert_eq!(requests.len(), 1, "expected exactly one request");
    assert_eq!(requests[0].method, "DELETE");
    assert_eq!(requests[0].path, "/vswitch/5");
    assert_eq!(requests[0].body, "cancellation_date=now");
}

#[test]
fn read_only_commands_reach_the_stub() {
    let stub = Stub::start();
    let output = run_cli(&stub, true, &["--json", "server", "list"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("alpha"));
    let requests = stub.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/server");
}

#[test]
fn missing_credentials_fail_without_a_request() {
    let stub = Stub::start();
    let output = run_cli(&stub, false, &["server", "list"]);

    assert!(!output.status.success());
    assert!(stub.requests().is_empty());
    assert!(stderr(&output).contains("missing credentials"));
}

#[test]
fn flags_supply_credentials_without_the_environment() {
    use std::io::Write as _;

    let password_path =
        std::env::temp_dir().join(format!("metalctl-e2e-pass-{}", std::process::id()));
    let mut file = std::fs::File::create(&password_path).expect("create password file");
    writeln!(file, "pass").expect("write password file");
    drop(file);

    let stub = Stub::start();
    let output = run_cli(
        &stub,
        false,
        &[
            "--user",
            "user",
            "--password-file",
            password_path.to_str().expect("utf-8 path"),
            "--json",
            "server",
            "list",
        ],
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let requests = stub.requests();
    // base64("user:pass") == "dXNlcjpwYXNz"
    assert_eq!(
        requests[0].authorization.as_deref(),
        Some("Basic dXNlcjpwYXNz")
    );
    let _ = std::fs::remove_file(&password_path);
}
