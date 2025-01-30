use cache::{DataCache, FileSystemDataCache};
use serde::Deserialize;
use std::collections::BTreeMap;

mod cache;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BrigadierJsonNodeType {
    Argument,
    Literal,
    Root,
}

#[derive(Debug, Deserialize)]
pub struct BrigadierJsonNode {
    #[serde(rename = "type")]
    pub node_type: BrigadierJsonNodeType,
    pub children: Option<BTreeMap<String, BrigadierJsonNode>>,
    pub executable: Option<bool>,
    pub parser: Option<String>,
    pub properties: Option<BTreeMap<String, serde_json::Value>>,
    pub redirect: Option<Vec<String>>,
}

pub struct DataProvider {
    cache: Box<dyn DataCache>,
}

impl DataProvider {
    pub fn new() -> Self {
        Self {
            cache: Box::new(FileSystemDataCache::new()),
        }
    }

    pub fn get_command_data(&self, version: String) -> BrigadierJsonNode {
        let data: String = self.cache.get(version);
        serde_json::from_str(&data).unwrap()
    }
}
