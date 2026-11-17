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

/// The document under test. Columns are chosen so an off-by-one shows up:
/// `total` is declared at column 4 and first used at column 8.
const SOURCE: &str = "\
fn total(a: int, b: int) -> int {
    return a + b;
}

fn main() -> int {
    let mut total: int = 0;
    print(total);
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

    // Line 6 (0-based) is `    let mut total: int = 0;` — `total` starts at
    // character 12. Put the cursor in the middle of the use on line 7 instead:
    // `    print(total);` — `total` starts at character 10.
    server.send(&format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"textDocument/definition","params":{{"textDocument":{{"uri":"{uri}"}},"position":{{"line":7,"character":12}}}}}}"#
    ));
    let response = server.response(2);
    let result = &response["result"];

    assert!(
        !result.is_null(),
        "definition should resolve `total`: {response}"
    );
    assert_eq!(result["uri"].as_str(), Some(uri));
    assert_eq!(
        (
            result["range"]["start"]["line"].as_u64(),
            result["range"]["start"]["character"].as_u64()
        ),
        (Some(6), Some(12)),
        "definition must point at the identifier, not column 0: {response}"
    );
    assert_eq!(
        (
            result["range"]["end"]["line"].as_u64(),
            result["range"]["end"]["character"].as_u64()
        ),
        (Some(6), Some(17)),
        "the range must be the width of `total`: {response}"
    );
}

#[test]
fn references_lists_every_occurrence_with_its_own_span() {
    let uri = "file:///tmp/nova_lsp_references.nova";
    let mut server = Server::start();
    open(&mut server, uri);

    // `total` appears three times: the unused function name on line 0, the
    // binding on line 6 and the use on line 7.
    server.send(&format!(
        r#"{{"jsonrpc":"2.0","id":3,"method":"textDocument/references","params":{{"textDocument":{{"uri":"{uri}"}},"position":{{"line":7,"character":12}},"context":{{"includeDeclaration":true}}}}}}"#
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
        vec![(0, 3), (6, 12), (7, 10)],
        "expected the declaration, the binding and the use, in order: {response}"
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
        r#"{{"jsonrpc":"2.0","id":4,"method":"textDocument/references","params":{{"textDocument":{{"uri":"{uri}"}},"position":{{"line":7,"character":12}},"context":{{"includeDeclaration":false}}}}}}"#
    ));
    let response = server.response(4);
    let locations = response["result"].as_array().expect("array");

    assert_eq!(
        locations.len(),
        2,
        "with includeDeclaration:false the binding and the use remain, the \
         function declaration does not: {response}"
    );
}
