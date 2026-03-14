mod api;
mod config;
mod error;

use api::{api_get, api_get_url};
use config::{get_config, load_config, save_config, ZendConfig};
use error::{handle_error, print_json, CliError, ZendError};

use clap::{Parser, Subcommand};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};

const VALID_TICKET_STATUSES: &[&str] = &["new", "open", "pending", "hold", "solved", "closed"];

const TICKET_SUMMARY_FIELDS: &[&str] = &[
    "id",
    "subject",
    "description",
    "status",
    "priority",
    "created_at",
    "updated_at",
    "tags",
    "requester_id",
    "assignee_id",
    "collaborator_ids",
    "follower_ids",
    "organization_id",
    "group_id",
    "type",
    "via",
    "url",
];

#[derive(Parser)]
#[command(name = "zend", version = "2.0.0", about = "Zendesk tickets CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Set up Zendesk credentials interactively
    Configure,
    /// Get a single ticket
    Ticket {
        id: String,
        /// Output full API response without field filtering
        #[arg(long)]
        raw: bool,
    },
    /// Find tickets for an assignee email
    Email {
        email: String,
        /// unresolved|all|new|open|pending|hold|solved|closed or comma-separated list
        #[arg(long, default_value = "unresolved")]
        status: String,
        /// Max tickets to return
        #[arg(long, default_value = "20")]
        limit: String,
        /// Sort order (asc|desc)
        #[arg(long, default_value = "desc")]
        sort: String,
    },
    /// Find tickets where the user is a follower but not the assignee
    Follower {
        email: String,
        /// unresolved|all|new|open|pending|hold|solved|closed or comma-separated list
        #[arg(long, default_value = "unresolved")]
        status: String,
        /// Max tickets to return
        #[arg(long, default_value = "20")]
        limit: String,
        /// Sort order (asc|desc)
        #[arg(long, default_value = "desc")]
        sort: String,
    },
    /// List slim comment timeline for a ticket
    Comments {
        #[arg(name = "ticketId")]
        ticket_id: String,
        /// all|public|private
        #[arg(long, default_value = "all")]
        visibility: String,
        /// Sort order (asc|desc)
        #[arg(long, default_value = "asc")]
        sort: String,
    },
}

fn is_email(s: &str) -> bool {
    let re = regex_lite::Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap();
    re.is_match(s)
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Preprocess argv to route bare arguments to the right subcommand.
fn preprocess_args(args: Vec<String>) -> Vec<String> {
    let known_commands: HashSet<&str> =
        ["configure", "follower", "comments", "help", "email", "ticket"]
            .iter()
            .copied()
            .collect();

    if args.len() < 2 {
        return args;
    }

    let first_arg = &args[1];
    if first_arg.starts_with('-') || known_commands.contains(first_arg.as_str()) {
        return args;
    }

    if is_email(first_arg) {
        let mut new_args = vec![args[0].clone(), "email".to_string()];
        new_args.extend_from_slice(&args[1..]);
        return new_args;
    }

    if is_digits(first_arg) {
        let mut new_args = vec![args[0].clone(), "ticket".to_string()];
        new_args.extend_from_slice(&args[1..]);
        return new_args;
    }

    args
}

fn parse_status_filter(input: &str) -> Result<Vec<String>, CliError> {
    let statuses: Vec<String> = input
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let invalid: Vec<&String> = statuses
        .iter()
        .filter(|s| *s != "unresolved" && *s != "all" && !VALID_TICKET_STATUSES.contains(&s.as_str()))
        .collect();

    if !invalid.is_empty() {
        let names: Vec<&str> = invalid.iter().map(|s| s.as_str()).collect();
        return Err(CliError::new("invalid_args", &format!("Invalid status value(s): {}", names.join(", ")))
            .with_details(json!({ "input": input })));
    }

    if statuses.contains(&"all".to_string()) {
        return Ok(vec![]);
    }
    if statuses.contains(&"unresolved".to_string()) {
        return Ok(vec![
            "new".to_string(),
            "open".to_string(),
            "pending".to_string(),
            "hold".to_string(),
        ]);
    }
    Ok(statuses)
}

fn parse_limit(input: &str) -> Result<usize, CliError> {
    let limit: usize = input.parse().map_err(|_| {
        CliError::new("invalid_args", "limit must be an integer").with_details(json!({ "limit": input }))
    })?;
    if limit < 1 || limit > 100 {
        return Err(CliError::new("invalid_args", "limit must be between 1 and 100")
            .with_details(json!({ "limit": limit })));
    }
    Ok(limit)
}

fn parse_sort(input: &str) -> Result<String, CliError> {
    if input != "asc" && input != "desc" {
        return Err(
            CliError::new("invalid_args", "sort must be asc or desc").with_details(json!({ "sort": input }))
        );
    }
    Ok(input.to_string())
}

fn parse_visibility(input: &str) -> Result<String, CliError> {
    if input != "all" && input != "public" && input != "private" {
        return Err(CliError::new("invalid_args", "visibility must be all, public, or private")
            .with_details(json!({ "visibility": input })));
    }
    Ok(input.to_string())
}

fn build_search_query(base: &str, raw_status: &str, status_filter: &[String]) -> String {
    if raw_status == "unresolved" {
        return format!("{base} status<solved");
    }
    if status_filter.len() == 1 {
        return format!("{base} status:{}", status_filter[0]);
    }
    base.to_string()
}

fn filter_statuses(tickets: Vec<Value>, raw_status: &str, status_filter: &[String]) -> Vec<Value> {
    if raw_status == "unresolved" || status_filter.len() <= 1 {
        return tickets;
    }
    tickets
        .into_iter()
        .filter(|t| {
            let s = t
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            status_filter.contains(&s)
        })
        .collect()
}

fn pick_ticket_fields(ticket: &Value) -> Value {
    let mut result = serde_json::Map::new();
    if let Value::Object(map) = ticket {
        for &key in TICKET_SUMMARY_FIELDS {
            if let Some(val) = map.get(key) {
                result.insert(key.to_string(), val.clone());
            }
        }
    }
    Value::Object(result)
}

async fn find_user_by_email(client: &Client, email: &str) -> Result<Value, ZendError> {
    let mut params = HashMap::new();
    params.insert("query".to_string(), email.to_string());
    let data = api_get(client, "/api/v2/users/search.json", &params).await?;
    let users = data.get("users").and_then(|v| v.as_array());
    if let Some(users) = users {
        // Try exact match first
        if let Some(exact) = users.iter().find(|u| {
            u.get("email")
                .and_then(|e| e.as_str())
                .map(|e| e.eq_ignore_ascii_case(email))
                .unwrap_or(false)
        }) {
            return Ok(exact.clone());
        }
        if let Some(first) = users.first() {
            return Ok(first.clone());
        }
    }
    Err(CliError::new("user_not_found", &format!("No Zendesk user found for email: {email}"))
        .with_details(json!({ "email": email }))
        .into())
}

async fn fetch_users_by_ids(client: &Client, ids: &[i64]) -> Result<HashMap<i64, Value>, ZendError> {
    let mut user_map = HashMap::new();
    let unique_ids: Vec<i64> = {
        let mut set = HashSet::new();
        ids.iter().filter(|id| set.insert(**id)).copied().collect()
    };

    for chunk in unique_ids.chunks(100) {
        let ids_str: Vec<String> = chunk.iter().map(|id| id.to_string()).collect();
        let mut params = HashMap::new();
        params.insert("ids".to_string(), ids_str.join(","));
        match api_get(client, "/api/v2/users/show_many.json", &params).await {
            Ok(data) => {
                if let Some(users) = data.get("users").and_then(|v| v.as_array()) {
                    for user in users {
                        if let Some(id) = user.get("id").and_then(|v| v.as_i64()) {
                            user_map.insert(id, user.clone());
                        }
                    }
                }
            }
            Err(e) => {
                if let Some(404) = e.status {
                    continue;
                }
                return Err(e.into());
            }
        }
    }

    Ok(user_map)
}

fn normalize_comment_body(comment: &Value) -> String {
    let body = comment
        .get("plain_body")
        .and_then(|v| v.as_str())
        .or_else(|| comment.get("body").and_then(|v| v.as_str()))
        .unwrap_or("");
    body.replace("&nbsp;", " ").replace('\r', "").trim().to_string()
}

fn to_slim_comment(comment: &Value, user_map: &HashMap<i64, Value>) -> Value {
    let author_id = comment.get("author_id").and_then(|v| v.as_i64());
    let author = match author_id {
        Some(id) => user_map
            .get(&id)
            .and_then(|u| u.get("name").and_then(|n| n.as_str()))
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("user:{id}")),
        None => "unknown".to_string(),
    };
    let time = comment
        .get("created_at")
        .and_then(|v| v.as_str())
        .map(|s| Value::String(s.to_string()))
        .unwrap_or(Value::Null);
    let visibility = if comment.get("public").and_then(|v| v.as_bool()).unwrap_or(false) {
        "public"
    } else {
        "private"
    };

    json!({
        "author": author,
        "time": time,
        "visibility": visibility,
        "body": normalize_comment_body(comment),
    })
}

fn prompt_line(question: &str, default: &str) -> String {
    let suffix = if default.is_empty() {
        String::new()
    } else {
        format!(" [{default}]")
    };
    eprint!("{question}{suffix}: ");
    io::stderr().flush().ok();
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line).ok();
    let trimmed = line.trim().to_string();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed
    }
}

fn prompt_hidden(question: &str, has_default: bool) -> String {
    let suffix = if has_default { " [****]" } else { "" };
    eprint!("{question}{suffix}: ");
    io::stderr().flush().ok();
    // For non-TTY or simple implementation, just read a line
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line).ok();
    line.trim().to_string()
}

async fn run_configure() -> Result<(), ZendError> {
    let existing = load_config();
    eprintln!("Zendesk CLI Configuration");
    eprintln!("{}", "─".repeat(30));

    let subdomain = prompt_line("Subdomain (xxx.zendesk.com)", &existing.subdomain);
    let email = prompt_line("Email", &existing.email);
    let token_input = prompt_hidden("API Token", !existing.api_token.is_empty());
    let api_token = if token_input.is_empty() {
        existing.api_token
    } else {
        token_input
    };

    let config = ZendConfig {
        subdomain,
        email,
        api_token,
    };
    save_config(&config).map_err(|e| ZendError::Other(e))?;
    print_json(&json!({ "ok": true }));
    Ok(())
}

async fn run_ticket(client: &Client, id: &str, raw: bool) -> Result<(), ZendError> {
    if !is_digits(id) {
        return Err(CliError::new("invalid_args", "ticket id must be numeric")
            .with_details(json!({ "id": id }))
            .into());
    }

    let path = format!("/api/v2/tickets/{id}.json");
    let params = HashMap::new();
    match api_get(client, &path, &params).await {
        Ok(data) => {
            let ticket = data.get("ticket");
            match ticket {
                Some(t) => {
                    if raw {
                        print_json(t);
                    } else {
                        print_json(&pick_ticket_fields(t));
                    }
                    Ok(())
                }
                None => Err(CliError::new("not_found", &format!("Ticket {id} not found"))
                    .with_details(json!({ "id": id.parse::<i64>().unwrap_or(0) }))
                    .into()),
            }
        }
        Err(e) => {
            if let Some(404) = e.status {
                return Err(CliError::new("not_found", &format!("Ticket {id} not found"))
                    .with_details(json!({ "id": id.parse::<i64>().unwrap_or(0) }))
                    .into());
            }
            Err(e.into())
        }
    }
}

async fn run_email(
    client: &Client,
    email: &str,
    raw_status: &str,
    limit_str: &str,
    sort_str: &str,
) -> Result<(), ZendError> {
    let limit = parse_limit(limit_str)?;
    let sort = parse_sort(sort_str)?;
    let status_filter = parse_status_filter(raw_status)?;
    let query = build_search_query(&format!("type:ticket assignee:{email}"), raw_status, &status_filter);

    let mut params = HashMap::new();
    params.insert("query".to_string(), query);
    params.insert("sort_by".to_string(), "updated_at".to_string());
    params.insert("sort_order".to_string(), sort);
    params.insert("per_page".to_string(), "100".to_string());

    let data = api_get(client, "/api/v2/search.json", &params).await?;
    let results = data
        .get("results")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let filtered = filter_statuses(results, raw_status, &status_filter);
    let limited: Vec<Value> = filtered.into_iter().take(limit).collect();
    print_json(&Value::Array(limited));
    Ok(())
}

async fn run_follower(
    client: &Client,
    email: &str,
    raw_status: &str,
    limit_str: &str,
    sort_str: &str,
) -> Result<(), ZendError> {
    let limit = parse_limit(limit_str)?;
    let sort = parse_sort(sort_str)?;
    let status_filter = parse_status_filter(raw_status)?;

    let user = find_user_by_email(client, email).await?;
    let user_id = user.get("id").and_then(|v| v.as_i64()).unwrap_or(0);

    let config = get_config().map_err(ZendError::Other)?;
    let base_url = format!("https://{}.zendesk.com/api/v2/search.json", config.subdomain);
    let query = build_search_query("type:ticket", raw_status, &status_filter);

    let query_str = format!(
        "{}?query={}&sort_by=updated_at&sort_order={}&per_page=100",
        base_url,
        urlencoding::encode(&query),
        sort
    );

    let mut matches: Vec<Value> = Vec::new();
    let mut next_url: Option<String> = Some(query_str);

    while let Some(url) = next_url.take() {
        if matches.len() >= limit {
            break;
        }
        let data = api_get_url(client, &url).await?;
        let results = data
            .get("results")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let tickets = filter_statuses(results, raw_status, &status_filter);

        for ticket in tickets {
            let follower_ids: Vec<i64> = ticket
                .get("follower_ids")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect())
                .unwrap_or_default();
            let assignee_id = ticket.get("assignee_id").and_then(|v| v.as_i64());

            if follower_ids.contains(&user_id) && assignee_id != Some(user_id) {
                matches.push(ticket);
            }
            if matches.len() >= limit {
                break;
            }
        }

        next_url = data
            .get("next_page")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }

    print_json(&Value::Array(matches));
    Ok(())
}

async fn run_comments(
    client: &Client,
    ticket_id: &str,
    visibility_str: &str,
    sort_str: &str,
) -> Result<(), ZendError> {
    if !is_digits(ticket_id) {
        return Err(CliError::new("invalid_args", "ticket id must be numeric")
            .with_details(json!({ "ticketId": ticket_id }))
            .into());
    }

    let sort = parse_sort(sort_str)?;
    let visibility = parse_visibility(visibility_str)?;

    let mut params = HashMap::new();
    params.insert("sort_order".to_string(), sort);
    params.insert("per_page".to_string(), "100".to_string());

    let path = format!("/api/v2/tickets/{ticket_id}/comments.json");
    let data = api_get(client, &path, &params).await?;
    let comments = data
        .get("comments")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Filter by visibility
    let filtered: Vec<Value> = match visibility.as_str() {
        "public" => comments
            .into_iter()
            .filter(|c| c.get("public").and_then(|v| v.as_bool()).unwrap_or(false))
            .collect(),
        "private" => comments
            .into_iter()
            .filter(|c| !c.get("public").and_then(|v| v.as_bool()).unwrap_or(false))
            .collect(),
        _ => comments,
    };

    // Collect author IDs
    let author_ids: Vec<i64> = filtered
        .iter()
        .filter_map(|c| c.get("author_id").and_then(|v| v.as_i64()))
        .collect();
    let user_map = fetch_users_by_ids(client, &author_ids).await?;

    let slim: Vec<Value> = filtered.iter().map(|c| to_slim_comment(c, &user_map)).collect();
    print_json(&Value::Array(slim));
    Ok(())
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let processed = preprocess_args(args);

    let cli = match Cli::try_parse_from(&processed) {
        Ok(cli) => cli,
        Err(e) => {
            // For help/version, just print and exit
            if e.kind() == clap::error::ErrorKind::DisplayHelp
                || e.kind() == clap::error::ErrorKind::DisplayVersion
            {
                print!("{e}");
                std::process::exit(0);
            }
            // For other errors, output structured JSON error
            error::fail(
                "invalid_args",
                &e.to_string(),
                json!({}),
                1,
            );
        }
    };

    let client = Client::new();

    let result = match cli.command {
        Commands::Configure => run_configure().await,
        Commands::Ticket { ref id, raw } => run_ticket(&client, id, raw).await,
        Commands::Email {
            ref email,
            ref status,
            ref limit,
            ref sort,
        } => run_email(&client, email, status, limit, sort).await,
        Commands::Follower {
            ref email,
            ref status,
            ref limit,
            ref sort,
        } => run_follower(&client, email, status, limit, sort).await,
        Commands::Comments {
            ref ticket_id,
            ref visibility,
            ref sort,
        } => run_comments(&client, ticket_id, visibility, sort).await,
    };

    if let Err(e) = result {
        handle_error(e);
    }
}
