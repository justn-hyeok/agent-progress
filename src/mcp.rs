//! Bounded JSON-RPC stdio MCP surface. No shell execution or global configuration writes.
use crate::{
    model::{Status, Verification},
    project::Project,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};

fn text(args: &Value, key: &str) -> Result<String> {
    let value = args[key]
        .as_str()
        .with_context(|| format!("missing string: {key}"))?;
    crate::model::nonempty(value, key)?;
    Ok(value.into())
}

fn tools(writes: bool) -> Vec<Value> {
    let mut tools = vec![
        json!({"name":"plan_read","description":"Read the shared product plan, statuses and evidence; does not observe process activity.","inputSchema":{"type":"object","properties":{},"additionalProperties":false},"annotations":{"readOnlyHint":true}}),
        json!({"name":"plan_resume","description":"Read last completion, recent changes, blockers and next action.","inputSchema":{"type":"object","properties":{},"additionalProperties":false},"annotations":{"readOnlyHint":true}}),
        json!({"name":"task_revision","description":"Capture declared code-input fingerprint before running a verification.","inputSchema":{"type":"object","properties":{"key":{"type":"string"}},"required":["key"],"additionalProperties":false},"annotations":{"readOnlyHint":true}}),
    ];
    if writes {
        for (name, description, properties, required) in [
            (
                "task_status",
                "Record explicit owner status. Completion still requires declared evidence.",
                json!({"key":{"type":"string"},"status":{"type":"string","enum":["planned","active","review","blocked","done","cancelled"]},"reason":{"type":"string"}}),
                vec!["key", "status", "reason"],
            ),
            (
                "task_note",
                "Record a decision/note and optional next action.",
                json!({"key":{"type":"string"},"text":{"type":"string"},"next":{"type":"string"}}),
                vec!["key", "text"],
            ),
            (
                "task_evidence",
                "Record a supplied evidence reference. Does not run tests or authenticate human verification.",
                json!({"key":{"type":"string"},"kind":{"type":"string","enum":["reported","automated","human"]},"reference":{"type":"string"},"revision":{"type":"string"}}),
                vec!["key", "kind", "reference"],
            ),
        ] {
            tools.push(json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},"annotations":{"readOnlyHint":false,"destructiveHint":true}}));
        }
    }
    tools
}

fn call(project: &Project, name: &str, args: &Value, writes: bool) -> Result<Value> {
    ensure!(args.is_object(), "arguments must be an object");
    let allowed: &[&str] = match name {
        "plan_read" | "plan_resume" => &[],
        "task_revision" => &["key"],
        "task_status" => &["key", "status", "reason"],
        "task_note" => &["key", "text", "next"],
        "task_evidence" => &["key", "kind", "reference", "revision"],
        _ => anyhow::bail!("unknown tool"),
    };
    ensure!(
        args.as_object()
            .unwrap()
            .keys()
            .all(|k| allowed.contains(&k.as_str())),
        "unknown argument"
    );
    match name {
        "plan_read" => Ok(serde_json::to_value(project.plan()?)?),
        "plan_resume" => project.resume(),
        "task_revision" => Ok(json!({"code_revision":project.code_revision(&text(args,"key")?)?})),
        _ => {
            ensure!(
                writes,
                "MCP writes disabled; start with --allow-writes to expose mutations"
            );
            let key = text(args, "key")?;
            let plan = match name {
                "task_status" => {
                    let status: Status = serde_json::from_value(args["status"].clone())?;
                    project.status(&key, status, "mcp", &text(args, "reason")?)?
                }
                "task_note" => project.note(
                    &key,
                    "mcp",
                    &text(args, "text")?,
                    args.get("next").map(|_| text(args, "next")).transpose()?,
                )?,
                "task_evidence" => {
                    let kind: Verification = serde_json::from_value(args["kind"].clone())?;
                    project.evidence_at(
                        &key,
                        kind,
                        "mcp",
                        &text(args, "reference")?,
                        args.get("revision")
                            .map(|_| text(args, "revision"))
                            .transpose()?
                            .as_deref(),
                    )?
                }
                _ => unreachable!(),
            };
            Ok(serde_json::to_value(plan)?)
        }
    }
}

pub fn serve(
    project: Project,
    writes: bool,
    input: impl BufRead,
    mut output: impl Write,
) -> Result<()> {
    let mut input = input;
    let mut negotiated = false;
    let mut initialized = false;
    loop {
        let mut line = String::new();
        let count = (&mut input).take(1024 * 1024 + 1).read_line(&mut line)?;
        if count == 0 {
            return Ok(());
        }
        ensure!(
            count <= 1024 * 1024 && line.ends_with('\n'),
            "MCP frame too large or incomplete"
        );
        let message: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => {
                writeln!(
                    output,
                    "{}",
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}})
                )?;
                output.flush()?;
                continue;
            }
        };
        let id = message.get("id");
        let method = message["method"].as_str().unwrap_or("");
        if id.is_none() {
            if negotiated && method == "notifications/initialized" && message["jsonrpc"] == "2.0" {
                initialized = true;
            }
            continue;
        }
        let id = id.unwrap();
        let result: Result<Value> = (|| {
            ensure!(
                message["jsonrpc"] == "2.0" && (id.is_string() || id.is_number()),
                "invalid JSON-RPC request"
            );
            match method {
                "initialize" => {
                    ensure!(!negotiated, "already initialized");
                    let version = text(&message["params"], "protocolVersion")?;
                    let supported = ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];
                    negotiated = true;
                    Ok(
                        json!({"protocolVersion":if supported.contains(&version.as_str()) {version.as_str()} else {"2025-11-25"},"capabilities":{"tools":{}},"serverInfo":{"name":"agent-progress","version":env!("CARGO_PKG_VERSION")}}),
                    )
                }
                "ping" => Ok(json!({})),
                _ => {
                    ensure!(initialized, "initialize handshake required");
                    match method {
                        "tools/list" => Ok(json!({"tools":tools(writes)})),
                        "tools/call" => {
                            let name = text(&message["params"], "name")?;
                            let args = message["params"]
                                .get("arguments")
                                .cloned()
                                .unwrap_or(json!({}));
                            Ok(match call(&project, &name, &args, writes) {
                                Ok(result) => {
                                    json!({"content":[{"type":"text","text":serde_json::to_string(&result)?}],"structuredContent":result,"isError":false})
                                }
                                Err(error) => {
                                    json!({"content":[{"type":"text","text":error.to_string()}],"isError":true})
                                }
                            })
                        }
                        _ => anyhow::bail!("method not found"),
                    }
                }
            }
        })();
        let response = match result {
            Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
            Err(error) => {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":if error.to_string()=="method not found" {-32601} else {-32600},"message":error.to_string()}})
            }
        };
        writeln!(output, "{response}")?;
        output.flush()?;
    }
}
