use serde_json::{json, Value};
use std::process;

/// CLI-level error with structured JSON output.
pub struct CliError {
    pub code: String,
    pub message: String,
    pub details: Value,
    pub exit_code: i32,
}

impl CliError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            details: json!({}),
            exit_code: 1,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }

    #[allow(dead_code)]
    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = code;
        self
    }
}

/// API-level error from HTTP requests.
pub struct ApiError {
    pub message: String,
    pub status: Option<u16>,
    pub body: Option<String>,
}

impl ApiError {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
            status: None,
            body: None,
        }
    }

    pub fn with_status(mut self, status: u16) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_body(mut self, body: String) -> Self {
        self.body = Some(body);
        self
    }
}

/// Unified error type for the CLI.
pub enum ZendError {
    Cli(CliError),
    Api(ApiError),
    Other(String),
}

impl From<CliError> for ZendError {
    fn from(e: CliError) -> Self {
        ZendError::Cli(e)
    }
}

impl From<ApiError> for ZendError {
    fn from(e: ApiError) -> Self {
        ZendError::Api(e)
    }
}

/// Print a JSON value to stdout with pretty formatting.
pub fn print_json(value: &Value) {
    println!("{}", serde_json::to_string_pretty(value).unwrap());
}

/// Print a structured error JSON to stdout and exit.
pub fn fail(code: &str, message: &str, details: Value, exit_code: i32) -> ! {
    let mut obj = json!({ "error": code, "message": message });
    if let Value::Object(map) = details {
        if let Value::Object(ref mut out) = obj {
            for (k, v) in map {
                out.insert(k, v);
            }
        }
    }
    print_json(&obj);
    process::exit(exit_code);
}

/// Handle any ZendError by printing structured JSON and exiting.
pub fn handle_error(error: ZendError) -> ! {
    match error {
        ZendError::Cli(e) => fail(&e.code, &e.message, e.details, e.exit_code),
        ZendError::Api(e) => {
            if let Some(status) = e.status {
                if status == 401 {
                    fail("auth_failed", "401 Unauthorized", json!({ "status": 401 }), 1);
                }
                if status == 404 {
                    fail("not_found", "Resource not found", json!({ "status": 404 }), 1);
                }
                let msg = e.body.as_deref().unwrap_or(&e.message);
                fail("api_error", msg, json!({ "status": status }), 1);
            }
            fail("api_error", &e.message, json!({}), 1);
        }
        ZendError::Other(msg) => fail("unknown_error", &msg, json!({}), 1),
    }
}
