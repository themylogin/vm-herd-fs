use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Value, json};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::mounts::{MountError, Registry};
use crate::view::Squash;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MountParams {
    pub src: PathBuf,
    pub overlay: Option<OverlayParams>,
    pub dst: PathBuf,
    pub squash: Option<Squash>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayParams {
    pub src: PathBuf,
    pub directories: Vec<String>,
}

struct RpcError {
    code: i64,
    message: String,
}

impl From<MountError> for RpcError {
    fn from(err: MountError) -> Self {
        match err {
            MountError::InvalidParams(message) => Self {
                code: -32602,
                message,
            },
            MountError::Failed(message) => Self {
                code: -32000,
                message,
            },
        }
    }
}

pub fn serve(listen: &str) -> Result<(), String> {
    let server = Server::http(listen).map_err(|err| format!("listen on {listen}: {err}"))?;
    println!("listening on {}", server.server_addr());
    let mut registry = Registry::default();
    for mut request in server.incoming_requests() {
        let response = handle(&mut registry, &mut request);
        if let Err(err) = request.respond(response) {
            eprintln!("send response: {err}");
        }
    }
    Ok(())
}

fn handle(registry: &mut Registry, request: &mut Request) -> Response<std::io::Cursor<Vec<u8>>> {
    if request.url() != "/api" {
        return Response::from_string("not found\n").with_status_code(404);
    }
    if *request.method() != Method::Post {
        return Response::from_string("method not allowed\n").with_status_code(405);
    }
    let mut body = String::new();
    let reply = match request.as_reader().read_to_string(&mut body) {
        Ok(_) => call(registry, &body),
        Err(_) => error_reply(Value::Null, -32700, "Parse error".into()),
    };
    let content_type =
        Header::from_bytes("Content-Type", "application/json").expect("valid header");
    Response::from_string(reply.to_string()).with_header(content_type)
}

fn call(registry: &mut Registry, body: &str) -> Value {
    let Ok(request) = serde_json::from_str::<Value>(body) else {
        return error_reply(Value::Null, -32700, "Parse error".into());
    };
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    if request.get("jsonrpc") != Some(&json!("2.0")) {
        return error_reply(id, -32600, "Invalid Request".into());
    }
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return error_reply(id, -32600, "Invalid Request".into());
    };
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "mount" => mount(registry, params),
        _ => Err(RpcError {
            code: -32601,
            message: "Method not found".into(),
        }),
    };
    match result {
        Ok(result) => json!({"jsonrpc": "2.0", "result": result, "id": id}),
        Err(err) => error_reply(id, err.code, err.message),
    }
}

fn mount(registry: &mut Registry, params: Value) -> Result<Value, RpcError> {
    let params: MountParams = serde_json::from_value(params).map_err(|err| RpcError {
        code: -32602,
        message: err.to_string(),
    })?;
    registry.mount(params)?;
    Ok(Value::Null)
}

fn error_reply(id: Value, code: i64, message: String) -> Value {
    json!({"jsonrpc": "2.0", "error": {"code": code, "message": message}, "id": id})
}
