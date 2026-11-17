//! Drives the real language server over stdio and checks navigation.
//!
//! Two contracts editors rely on, both of which were previously fake:
//!
//! - `textDocument/definition` returned the *whole line*, starting at column
//!   zero, for any line containing the text `fn <name>`. It now returns the
//!   identifier's own span.
//! - `textDocument/references` returned `null` while the server advertised
//!   `referencesProvider: false`. It now answers, and the capability is on.
//!
//! The ranges are exact, so the assertions can be exact: a column that is off by
//! one is a bug an editor shows the user, not a detail.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
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

struct Server {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<String>,
}

impl Server {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_nova-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start nova-lsp");

        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let (tx, rx): (Sender<String>, Receiver<String>) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Some(message) = read_message(&mut reader) {
                if tx.send(message).is_err() {
                    break;
                }
            }
        });

        Server { child, stdin, rx }
    }

    fn send(&mut self, body: &str) {
        self.stdin
            .write_all(frame(body).as_bytes())
            .expect("write message");
    }

    /// Read until a response with this id arrives, skipping notifications.
    fn response(&self, id: u32) -> serde_json::Value {
        loop {
            let message = self
                .rx
                .recv_timeout(Duration::from_secs(20))
                .unwrap_or_else(|_| panic!("no response to request {id}"));
            let json: serde_json::Value = serde_json::from_str(&message).expect("valid JSON-RPC");
            if json["id"].as_u64() == Some(u64::from(id)) {
                return json;
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The document under test.
///
/// Every column below is counted, not guessed, and the names do not shadow each
/// other — a token-level search treats two identifiers with the same text as one
/// symbol, so a shadowing document would test the limitation rather than the
/// feature.
///
/// ```text
/// 0  fn total(a: int, b: int) -> int {
/// 1      return a + b;
/// 2  }
/// 3
/// 4  fn main() -> int {
/// 5      let start: int = 0;
/// 6      let sum: int = total(start, 2);
/// 7      print(sum);
/// 8      print(start);
/// 9      return 0;
/// 10 }
/// ```
const SOURCE: &str = "\
fn total(a: int, b: int) -> int {
    return a + b;
}

fn main() -> int {
    let start: int = 0;
    let sum: int = total(start, 2);
    print(sum);
    print(start);
    return 0;
}
";

fn open(server: &mut Server, uri: &str) {
    server.send(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}"#);
    let initialized = server.response(1);
    assert!(
        initialized["result"]["capabilities"]["referencesProvider"].as_bool() == Some(true),
        "references must be advertised now that it is implemented: {initialized}"
    );

    server.send(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#);
    server.send(&format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{uri}","languageId":"nova","version":1,"text":{}}}}}}}"#,
        serde_json::to_string(SOURCE).unwrap()
    ));

    // `didOpen` publishes diagnostics asynchronously; wait for it so the document
    // is definitely in the server's map before asking it anything.
    loop {
        let message = server
            .rx
            .recv_timeout(Duration::from_secs(20))
            .expect("publishDiagnostics after didOpen");
        let json: serde_json::Value = serde_json::from_str(&message).expect("valid JSON-RPC");
        if json["method"] == "textDocument/publishDiagnostics" {
            break;
        }
    }

    // A document with no errors must still be registered, so this returns
    // without inspecting the diagnostics: the point is that `didOpen` completed.
    let _ = initialized;
}

#[test]
fn definition_returns_the_identifier_span_not_the_line() {
    let uri = "file:///tmp/nova_lsp_definition.nova";
    let mut server = Server::start();
    open(&mut server, uri);

    // Cursor inside `total` on line 6 — `    let sum: int = total(start, 2);`
    // puts `total` at columns 19..24.
    server.send(&format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"textDocument/definition","params":{{"textDocument":{{"uri":"{uri}"}},"position":{{"line":6,"character":21}}}}}}"#
    ));
    let response = server.response(2);
    let result = &response["result"];

    assert!(!result.is_null(), "definition should resolve `total`: {response}");
    assert_eq!(result["uri"].as_str(), Some(uri));
    assert_eq!(
        (
            result["range"]["start"]["line"].as_u64(),
            result["range"]["start"]["character"].as_u64()
        ),
        (Some(0), Some(3)),
        "definition must point at the identifier, not column 0: {response}"
    );
    assert_eq!(
        (
            result["range"]["end"]["line"].as_u64(),
            result["range"]["end"]["character"].as_u64()
        ),
        (Some(0), Some(8)),
        "the range must be the width of `total`: {response}"
    );
}

#[test]
fn definition_resolves_a_local_binding() {
    let uri = "file:///tmp/nova_lsp_definition_local.nova";
    let mut server = Server::start();
    open(&mut server, uri);

    // Cursor inside `start` on line 8 — `    print(start);` puts it at 10..15.
    server.send(&format!(
        r#"{{"jsonrpc":"2.0","id":5,"method":"textDocument/definition","params":{{"textDocument":{{"uri":"{uri}"}},"position":{{"line":8,"character":12}}}}}}"#
    ));
    let response = server.response(5);
    let result = &response["result"];

    assert_eq!(
        (
            result["range"]["start"]["line"].as_u64(),
            result["range"]["start"]["character"].as_u64()
        ),
        (Some(5), Some(8)),
        "a local must resolve to its `let`, on line 5 column 8: {response}"
    );
}

#[test]
fn references_lists_every_occurrence_with_its_own_span() {
    let uri = "file:///tmp/nova_lsp_references.nova";
    let mut server = Server::start();
    open(&mut server, uri);

    // `start` appears three times: bound on line 5, passed on line 6, printed on
    // line 8.
    server.send(&format!(
        r#"{{"jsonrpc":"2.0","id":3,"method":"textDocument/references","params":{{"textDocument":{{"uri":"{uri}"}},"position":{{"line":8,"character":12}},"context":{{"includeDeclaration":true}}}}}}"#
    ));
    let response = server.response(3);
    let locations = response["result"]
        .as_array()
        .unwrap_or_else(|| panic!("references should be an array: {response}"));

    let starts: Vec<(u64, u64)> = locations
        .iter()
        .map(|location| {
            (
                location["range"]["start"]["line"].as_u64().unwrap(),
                location["range"]["start"]["character"].as_u64().unwrap(),
            )
        })
        .collect();

    assert_eq!(
        starts,
        vec![(5, 8), (6, 25), (8, 10)],
        "expected the binding, the argument and the use, in order: {response}"
    );

    // Every range must be the width of the identifier, never a whole line.
    for location in locations {
        let start = location["range"]["start"]["character"].as_u64().unwrap();
        let end = location["range"]["end"]["character"].as_u64().unwrap();
        assert_eq!(end - start, 5, "each range must be 5 characters: {response}");
    }
}

#[test]
fn references_can_omit_the_declaration() {
    let uri = "file:///tmp/nova_lsp_references_nodecl.nova";
    let mut server = Server::start();
    open(&mut server, uri);

    server.send(&format!(
        r#"{{"jsonrpc":"2.0","id":4,"method":"textDocument/references","params":{{"textDocument":{{"uri":"{uri}"}},"position":{{"line":8,"character":12}},"context":{{"includeDeclaration":false}}}}}}"#
    ));
    let response = server.response(4);
    let locations = response["result"].as_array().expect("array");

    assert_eq!(
        locations.len(),
        2,
        "with includeDeclaration:false the argument and the use remain, the \
         binding does not: {response}"
    );
}
