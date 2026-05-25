use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ItemsResponse {
    pub items: Vec<JellyfinItem>,
    pub total_record_count: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinItem {
    pub name: String,
    pub id: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub series_name: Option<String>,
    #[serde(default)]
    pub parent_index_number: Option<u32>,
    #[serde(default)]
    pub index_number: Option<u32>,
}

impl JellyfinItem {
    pub fn display_name(&self) -> String {
        if let Some(ref series) = self.series_name {
            let s = self.parent_index_number.map_or(String::new(), |n| format!("S{n:02}"));
            let e = self.index_number.map_or(String::new(), |n| format!("E{n:02}"));
            format!("{series} {s}{e} - {}", self.name)
        } else {
            self.name.clone()
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinUser {
    pub name: String,
    pub id: String,
    #[serde(default)]
    pub has_password: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LibraryInfo {
    pub name: String,
    pub collection_type: Option<String>,
    pub locations: Vec<String>,
}
