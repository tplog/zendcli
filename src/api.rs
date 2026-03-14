use crate::config::get_config;
use crate::error::ApiError;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;

fn auth_header() -> Result<String, String> {
    let config = get_config()?;
    let credentials = format!("{}/token:{}", config.email, config.api_token);
    let encoded = STANDARD.encode(credentials.as_bytes());
    Ok(format!("Basic {encoded}"))
}

fn base_url() -> Result<String, String> {
    let config = get_config()?;
    Ok(format!("https://{}.zendesk.com", config.subdomain))
}

async fn fetch_json(client: &Client, url: &str) -> Result<Value, ApiError> {
    let auth = auth_header().map_err(|e| ApiError::new(&e))?;

    let resp = client
        .get(url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| ApiError::new(&e.to_string()))?;

    let status = resp.status().as_u16();
    if status < 200 || status >= 300 {
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::new(&format!("HTTP {status}"))
            .with_status(status)
            .with_body(body));
    }

    resp.json::<Value>()
        .await
        .map_err(|e| ApiError::new(&e.to_string()))
}

pub async fn api_get(
    client: &Client,
    path: &str,
    params: &HashMap<String, String>,
) -> Result<Value, ApiError> {
    let base = base_url().map_err(|e| ApiError::new(&e))?;
    let mut url = format!("{base}{path}");

    if !params.is_empty() {
        let query: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect();
        url = format!("{url}?{}", query.join("&"));
    }

    fetch_json(client, &url).await
}

pub async fn api_get_url(client: &Client, url: &str) -> Result<Value, ApiError> {
    fetch_json(client, url).await
}
