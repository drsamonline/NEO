//! Drives the real language server over stdio and checks the diagnostics it
//! publishes for a document with an undefined name.
//!
//! This is the end-to-end contract editors rely on: `textDocument/didOpen`
//! produces a `textDocument/publishDiagnostics` notification whose range points
//! at the offending line and whose message states the cause.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

fn frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

fn read_message(reader: &mut BufReader<impl Read>) -> Option<String> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length: ") {
            length = value.parse::<usize>().ok();
        }
    }
    let mut body = vec![0u8; length?];
    reader.read_exact(&mut body).ok()?;
    String::from_utf8(body).ok()
}

#[test]
fn publishes_error_range_and_cause_for_undefined_name() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nova-lsp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start nova-lsp");

    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        while let Some(message) = read_message(&mut reader) {
            if tx.send(message).is_err() {
                break;
            }
        }
    });

    let initialize = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}"#;
    stdin
        .write_all(frame(initialize).as_bytes())
        .expect("write initialize");

    let initialized = rx
        .recv_timeout(Duration::from_secs(20))
        .expect("initialize response");
    assert!(
        initialized.contains("\"capabilities\""),
        "unexpected initialize response: {initialized}"
    );

    // Advertised sync mode must match `did_change`: this server replaces whole
    // documents, so it must ask for FULL sync.
    assert!(
        initialized.contains("\"textDocumentSync\":1"),
        "expected FULL sync in capabilities: {initialized}"
    );

    stdin
        .write_all(frame(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#).as_bytes())
        .expect("write initialized");

    let uri = "file:///tmp/nova_lsp_diagnostics.nova";
    let source = "fn main() {\n    missing();\n}\n";
    let did_open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{uri}","languageId":"nova","version":1,"text":{}}}}}}}"#,
        serde_json::to_string(source).unwrap()
    );
    stdin
        .write_all(frame(&did_open).as_bytes())
        .expect("write didOpen");

    // The server may log before publishing, so read until the notification.
    let json = loop {
        let message = rx
            .recv_timeout(Duration::from_secs(20))
            .expect("publishDiagnostics notification");
        let json: serde_json::Value = serde_json::from_str(&message).expect("valid JSON-RPC");
        if json["method"] == "textDocument/publishDiagnostics" {
            break json;
        }
    };
    let published = json.to_string();
    let diagnostics = json["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics array");
    let diagnostic = diagnostics
        .iter()
        .find(|d| d["severity"] == 1)
        .unwrap_or_else(|| panic!("no error diagnostic in {published}"));

    assert_eq!(
        diagnostic["range"]["start"]["line"], 1,
        "error should be on line 2 (0-based 1): {published}"
    );
    let message = diagnostic["message"].as_str().expect("message");
    assert!(
        message.contains("undefined name `missing`"),
        "message should name the cause: {message}"
    );
    assert!(
        message.contains("help:"),
        "message should suggest a fix: {message}"
    );

    // The server is a long-lived process; closing its stdin is enough for the
    // test, and killing avoids waiting on a task that may still be draining.
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
}
