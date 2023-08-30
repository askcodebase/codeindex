use std::fmt::Debug;

use schemars::JsonSchema;
use serde;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct VersionInfo {
    pub title: String,
    pub version: String,
}

impl Default for VersionInfo {
    fn default() -> Self {
        VersionInfo {
            title: "qdrant - vector search engine".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl VersionInfo {
    pub fn minor_version(&self) -> String {
        let minor = self
            .version
            .split('.')
            .take(2)
            .collect::<Vec<&str>>()
            .join(".");
        format!("{}.x", minor)
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApiStatus {
    Ok,
    Error(String),
    Accepted,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ApiResponse<D: Serialize + Debug> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<D>,
    pub status: ApiStatus,
    pub time: f64,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct CollectionDescription {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct CollectionsResponse {
    pub collections: Vec<CollectionDescription>,
}
