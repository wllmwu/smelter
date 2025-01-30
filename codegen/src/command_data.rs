use cache::{CacheError, DataCache, FileSystemDataCache};
use serde::Deserialize;
use std::{collections::BTreeMap, error, fmt};

mod cache;

/* * * * Public interface * * * */

#[derive(Debug)]
pub enum DataProviderError {
    DeserializationError(serde_json::Error),
    RetrieveError(CacheError),
}

#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BrigadierJsonNodeType {
    Argument,
    Literal,
    Root,
}

#[derive(Deserialize)]
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

    pub fn get_command_data(
        &self,
        version: String,
    ) -> Result<BrigadierJsonNode, DataProviderError> {
        let data: String = self.cache.get(version)?;
        Ok(serde_json::from_str(&data)?)
    }
}

/* * * * Private implementation * * * */

impl fmt::Display for DataProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeserializationError(e) => {
                write!(f, "JSON deserialization failed: {e}")
            }
            Self::RetrieveError(e) => {
                write!(f, "data retrieval failed: {e}")
            }
        }
    }
}

impl error::Error for DataProviderError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::DeserializationError(e) => Some(e),
            Self::RetrieveError(e) => Some(e),
        }
    }
}

impl From<CacheError> for DataProviderError {
    fn from(value: CacheError) -> Self {
        Self::RetrieveError(value)
    }
}

impl From<serde_json::Error> for DataProviderError {
    fn from(value: serde_json::Error) -> Self {
        Self::DeserializationError(value)
    }
}
