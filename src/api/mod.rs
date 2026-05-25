pub mod models;

use crate::config::JellyfinConfig;
use models::{ItemsResponse, JellyfinItem, JellyfinUser, LibraryInfo};
use reqwest::Client;

pub struct JellyfinApi {
    client: Client,
    base_url: String,
    api_key: String,
}

impl JellyfinApi {
    pub fn new(config: &JellyfinConfig) -> Self {
        Self {
            client: Client::new(),
            base_url: config.url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
        }
    }

    fn auth_header(&self) -> String {
        format!("MediaBrowser Token={}", self.api_key)
    }

    pub async fn get_libraries(&self) -> Result<Vec<LibraryInfo>, reqwest::Error> {
        self.client
            .get(format!("{}/Library/VirtualFolders", self.base_url))
            .header("Authorization", self.auth_header())
            .send()
            .await?
            .json()
            .await
    }

    pub async fn get_all_items(
        &self,
        item_types: &str,
        fields: &str,
    ) -> Result<Vec<JellyfinItem>, reqwest::Error> {
        let mut all_items = Vec::new();
        let limit = 200;
        let mut start = 0;

        loop {
            let resp: ItemsResponse = self
                .client
                .get(format!("{}/Items", self.base_url))
                .header("Authorization", self.auth_header())
                .query(&[
                    ("Recursive", "true"),
                    ("IncludeItemTypes", item_types),
                    ("Fields", fields),
                    ("Limit", &limit.to_string()),
                    ("StartIndex", &start.to_string()),
                ])
                .send()
                .await?
                .json()
                .await?;

            let count = resp.items.len();
            all_items.extend(resp.items);

            if count < limit || all_items.len() as u64 >= resp.total_record_count {
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
    ) -> Result<Vec<JellyfinItem>, reqwest::Error> {
        let resp: ItemsResponse = self
            .client
            .get(format!("{}/Items", self.base_url))
            .header("Authorization", self.auth_header())
            .query(&[
                ("Recursive", "true"),
                ("IncludeItemTypes", item_types),
                ("Fields", "Path,Tags"),
                ("Tags", tag),
            ])
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.items)
    }

    pub async fn add_tag(&self, item_id: &str, tag: &str) -> Result<(), reqwest::Error> {
        self.client
            .post(format!("{}/Items/{item_id}/Tags/Add", self.base_url))
            .header("Authorization", self.auth_header())
            .json(&serde_json::json!({ "Tags": [{ "Name": tag }] }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn remove_tag(&self, item_id: &str, tag: &str) -> Result<(), reqwest::Error> {
        self.client
            .post(format!("{}/Items/{item_id}/Tags/Delete", self.base_url))
            .header("Authorization", self.auth_header())
            .json(&serde_json::json!({ "Tags": [{ "Name": tag }] }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn get_item(&self, item_id: &str) -> Result<JellyfinItem, reqwest::Error> {
        self.client
            .get(format!("{}/Items/{item_id}", self.base_url))
            .header("Authorization", self.auth_header())
            .query(&[("Fields", "Path,Tags")])
            .send()
            .await?
            .json()
            .await
    }

    pub async fn get_users(&self) -> Result<Vec<JellyfinUser>, reqwest::Error> {
        self.client
            .get(format!("{}/Users", self.base_url))
            .header("Authorization", self.auth_header())
            .send()
            .await?
            .json()
            .await
    }
}
