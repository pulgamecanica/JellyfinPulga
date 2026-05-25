pub mod models;

use crate::config::JellyfinConfig;
use colored::Colorize;
use models::{ItemsResponse, JellyfinItem, JellyfinUser, LibraryInfo, ServerInfo};
use reqwest::{Client, RequestBuilder, Response};
use std::time::Duration;

const MIN_JELLYFIN_VERSION: (u32, u32) = (10, 9);
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 2000;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct JellyfinApi {
    client: Client,
    base_url: String,
    api_key: String,
}

#[derive(Debug)]
pub enum ApiError {
    Unreachable(String),
    Unauthorized,
    VersionMismatch { found: String, minimum: String },
    ServerError(u16, String),
    ParseError(String),
    Other(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(url) => write!(f, "Jellyfin server unreachable at {url}"),
            Self::Unauthorized => write!(f, "API key rejected — check your config.toml"),
            Self::VersionMismatch { found, minimum } => {
                write!(f, "Jellyfin version {found} is too old (minimum: {minimum})")
            }
            Self::ServerError(code, msg) => write!(f, "Jellyfin returned HTTP {code}: {msg}"),
            Self::ParseError(msg) => write!(f, "unexpected response format: {msg}"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl JellyfinApi {
    pub fn new(config: &JellyfinConfig) -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .expect("failed to build HTTP client");

        Self {
            client,
            base_url: config.url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
        }
    }

    fn auth_header(&self) -> String {
        format!("MediaBrowser Token={}", self.api_key)
    }

    async fn send_with_retry(&self, build_request: impl Fn() -> RequestBuilder) -> Result<Response, ApiError> {
        let mut last_err = None;

        for attempt in 1..=MAX_RETRIES {
            match build_request().send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status == reqwest::StatusCode::UNAUTHORIZED {
                        return Err(ApiError::Unauthorized);
                    }
                    if status.is_server_error() {
                        let body = resp.text().await.unwrap_or_default();
                        if attempt < MAX_RETRIES {
                            eprintln!(
                                "  {} Server error (HTTP {status}), retry {attempt}/{MAX_RETRIES}...",
                                "Warning:".yellow()
                            );
                            tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS * attempt as u64)).await;
                            last_err = Some(ApiError::ServerError(status.as_u16(), body));
                            continue;
                        }
                        return Err(ApiError::ServerError(status.as_u16(), body));
                    }
                    return Ok(resp);
                }
                Err(e) => {
                    if e.is_connect() || e.is_timeout() {
                        if attempt < MAX_RETRIES {
                            eprintln!(
                                "  {} Connection failed, retry {attempt}/{MAX_RETRIES}...",
                                "Warning:".yellow()
                            );
                            tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS * attempt as u64)).await;
                            last_err = Some(ApiError::Unreachable(self.base_url.clone()));
                            continue;
                        }
                        return Err(ApiError::Unreachable(self.base_url.clone()));
                    }
                    return Err(ApiError::Other(e.to_string()));
                }
            }
        }

        Err(last_err.unwrap_or_else(|| ApiError::Other("max retries exceeded".into())))
    }

    pub async fn health_check(&self) -> Result<ServerInfo, ApiError> {
        let resp = self.send_with_retry(|| {
            self.client
                .get(format!("{}/System/Info/Public", self.base_url))
        }).await?;

        let info: ServerInfo = resp.json().await
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        let parts: Vec<u32> = info.version.split('.').filter_map(|s| s.parse().ok()).collect();
        if parts.len() >= 2 {
            let (major, minor) = (parts[0], parts[1]);
            if (major, minor) < MIN_JELLYFIN_VERSION {
                return Err(ApiError::VersionMismatch {
                    found: info.version.clone(),
                    minimum: format!("{}.{}", MIN_JELLYFIN_VERSION.0, MIN_JELLYFIN_VERSION.1),
                });
            }
        }

        Ok(info)
    }

    #[allow(dead_code)]
    pub async fn get_libraries(&self) -> Result<Vec<LibraryInfo>, ApiError> {
        let resp = self.send_with_retry(|| {
            self.client
                .get(format!("{}/Library/VirtualFolders", self.base_url))
                .header("Authorization", self.auth_header())
        }).await?;

        resp.json().await.map_err(|e| ApiError::ParseError(e.to_string()))
    }

    pub async fn get_all_items(
        &self,
        item_types: &str,
        fields: &str,
    ) -> Result<Vec<JellyfinItem>, ApiError> {
        let mut all_items = Vec::new();
        let limit: usize = 200;
        let mut start: usize = 0;

        loop {
            let limit_str = limit.to_string();
            let start_str = start.to_string();
            let resp = self.send_with_retry(|| {
                self.client
                    .get(format!("{}/Items", self.base_url))
                    .header("Authorization", self.auth_header())
                    .query(&[
                        ("Recursive", "true"),
                        ("IncludeItemTypes", item_types),
                        ("Fields", fields),
                        ("Limit", &limit_str),
                        ("StartIndex", &start_str),
                    ])
            }).await?;

            let items_resp: ItemsResponse = resp.json().await
                .map_err(|e| ApiError::ParseError(e.to_string()))?;

            let count = items_resp.items.len();
            let total = items_resp.total_record_count;
            all_items.extend(items_resp.items);

            if count < limit || all_items.len() as u64 >= total {
                break;
            }
            start += limit;
        }

        Ok(all_items)
    }

    pub async fn get_items_by_tag(
        &self,
        tag: &str,
        item_types: &str,
    ) -> Result<Vec<JellyfinItem>, ApiError> {
        let resp = self.send_with_retry(|| {
            self.client
                .get(format!("{}/Items", self.base_url))
                .header("Authorization", self.auth_header())
                .query(&[
                    ("Recursive", "true"),
                    ("IncludeItemTypes", item_types),
                    ("Fields", "Path,Tags"),
                    ("Tags", tag),
                ])
        }).await?;

        let items_resp: ItemsResponse = resp.json().await
            .map_err(|e| ApiError::ParseError(e.to_string()))?;
        Ok(items_resp.items)
    }

    pub async fn add_tag(&self, item_id: &str, tag: &str) -> Result<(), ApiError> {
        let url = format!("{}/Items/{item_id}/Tags/Add", self.base_url);
        let body = serde_json::json!({ "Tags": [{ "Name": tag }] });
        let resp = self.send_with_retry(|| {
            self.client
                .post(&url)
                .header("Authorization", self.auth_header())
                .json(&body)
        }).await?;

        if let Err(e) = resp.error_for_status_ref() {
            return Err(ApiError::ServerError(e.status().map_or(0, |s| s.as_u16()), e.to_string()));
        }
        Ok(())
    }

    pub async fn remove_tag(&self, item_id: &str, tag: &str) -> Result<(), ApiError> {
        let url = format!("{}/Items/{item_id}/Tags/Delete", self.base_url);
        let body = serde_json::json!({ "Tags": [{ "Name": tag }] });
        let resp = self.send_with_retry(|| {
            self.client
                .post(&url)
                .header("Authorization", self.auth_header())
                .json(&body)
        }).await?;

        if let Err(e) = resp.error_for_status_ref() {
            return Err(ApiError::ServerError(e.status().map_or(0, |s| s.as_u16()), e.to_string()));
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn get_item(&self, item_id: &str) -> Result<JellyfinItem, ApiError> {
        let resp = self.send_with_retry(|| {
            self.client
                .get(format!("{}/Items/{item_id}", self.base_url))
                .header("Authorization", self.auth_header())
                .query(&[("Fields", "Path,Tags")])
        }).await?;

        resp.json().await.map_err(|e| ApiError::ParseError(e.to_string()))
    }

    pub async fn get_users(&self) -> Result<Vec<JellyfinUser>, ApiError> {
        let resp = self.send_with_retry(|| {
            self.client
                .get(format!("{}/Users", self.base_url))
                .header("Authorization", self.auth_header())
        }).await?;

        resp.json().await.map_err(|e| ApiError::ParseError(e.to_string()))
    }
}
