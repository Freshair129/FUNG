use std::io::{self, BufRead, Write};
use std::time::Duration;

fn send(value: &str) {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{value}").expect("write fixture response");
    stdout.flush().expect("flush fixture response");
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "normal".into());
    if let Some(counter_path) = std::env::args().nth(2) {
        let count = std::fs::read_to_string(&counter_path)
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0)
            + 1;
        std::fs::write(counter_path, count.to_string()).expect("write fixture process count");
    }
    for line in io::stdin().lock().lines() {
        let line = line.expect("read fixture request");
        if line.contains("\"method\":\"initialize\"") {
            send(
                r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"fung-fixture","version":"1.0.0"}}}"#,
            );
        } else if line.contains("\"method\":\"notifications/initialized\"") {
            continue;
        } else if line.contains("\"method\":\"tools/list\"") {
            if mode == "crm" {
                send(
                    r#"{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"get_customer_status","description":"Read approved CRM status fields","inputSchema":{"type":"object","properties":{"customerKey":{"type":"string"},"fields":{"type":"array","items":{"type":"string"}}},"required":["customerKey","fields"],"additionalProperties":false}}]}}"#,
                );
            } else if mode == "write-advertised" {
                send(
                    r#"{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"search_documents","description":"Search approved metadata","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false}},{"name":"delete_documents","description":"Unapproved write","inputSchema":{"type":"object"}}]}}"#,
                );
            } else {
                send(
                    r#"{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"search_documents","description":"Search approved metadata","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false}}]}}"#,
                );
            }
        } else if line.contains("\"method\":\"tools/call\"") {
            if mode == "exit-on-call" {
                return;
            }
            if mode == "slow" {
                std::thread::sleep(Duration::from_secs(5));
            }
            if mode == "deep-result" {
                send(
                    r#"{"jsonrpc":"2.0","id":3,"result":{"content":[],"structuredContent":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"value":"too deep"}}}}}}}}}}}}}}}}}},"sourceRefs":["kb://documents/42"],"isError":false}}"#,
                );
                continue;
            }
            if mode == "crm" {
                send(
                    r#"{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"Active customer"}],"structuredContent":{"customerKey":"customer-42","status":"active","stage":"renewal"},"sourceRefs":["crm://customers/customer-42"],"isError":false}}"#,
                );
                continue;
            }
            send(
                r#"{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"Approved contract"}],"structuredContent":{"items":[{"title":"Approved contract","location":"kb://documents/42"}]},"sourceRefs":["kb://documents/42"],"isError":false}}"#,
            );
        }
    }
}
