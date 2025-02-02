use std::{error, fmt};

use brigadier_json::{BrigadierJsonNode, ConversionError};
use cache::{CacheError, DataCache, FileSystemDataCache};

use crate::brigadier_tree::BrigadierTree;

mod brigadier_json;
mod cache;
mod postprocessors;

/* * * * Public interface * * * */

#[derive(Debug)]
pub enum TreeProviderError {
    ConversionError(ConversionError),
    RetrieveError(CacheError),
}

pub struct TreeProvider {
    cache: Box<dyn DataCache>,
}

impl TreeProvider {
    pub fn new() -> Self {
        Self {
            cache: Box::new(FileSystemDataCache::new()),
        }
    }

    pub fn get_command_tree(&self, version: String) -> Result<BrigadierTree, TreeProviderError> {
        let data: String = self.cache.get(version)?;
        let json: BrigadierJsonNode = BrigadierJsonNode::try_from(data)?;
        let tree: BrigadierTree = BrigadierTree::try_from(json)?;
        Ok(postprocessors::consolidate_literals_into_enums(tree))
    }
}

/* * * * Private implementation * * * */

impl fmt::Display for TreeProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConversionError(e) => {
                write!(f, "conversion from JSON to BrigadierTree failed: {e}")
            }
            Self::RetrieveError(e) => {
                write!(f, "data retrieval failed: {e}")
            }
        }
    }
}

impl error::Error for TreeProviderError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::ConversionError(e) => Some(e),
            Self::RetrieveError(e) => Some(e),
        }
    }
}

impl From<CacheError> for TreeProviderError {
    fn from(value: CacheError) -> Self {
        Self::RetrieveError(value)
    }
}

impl From<ConversionError> for TreeProviderError {
    fn from(value: ConversionError) -> Self {
        Self::ConversionError(value)
    }
}
