use crate::brigadier_tree::{BrigadierTree, BrigadierTreeNode, BrigadierTreeNodeChildren};
use cache::{CacheError, DataCache, FileSystemDataCache};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    error, fmt,
};

mod cache;

/* * * * Public interface * * * */

#[derive(Debug)]
pub enum DataProviderError {
    ConversionError(String),
    DeserializationError(serde_json::Error),
    RetrieveError(CacheError),
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

    pub fn get_command_data(&self, version: String) -> Result<BrigadierTree, DataProviderError> {
        let data: String = self.cache.get(version)?;
        let json: BrigadierJsonNode = serde_json::from_str(&data)?;
        Ok(BrigadierTree::try_from(json)?)
    }
}

/* * * * Private implementation * * * */

#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum BrigadierJsonNodeType {
    Argument,
    Literal,
    Root,
}

#[derive(Deserialize)]
struct BrigadierJsonNode {
    #[serde(rename = "type")]
    node_type: BrigadierJsonNodeType,
    children: Option<BTreeMap<String, BrigadierJsonNode>>,
    executable: Option<bool>,
    parser: Option<String>,
    properties: Option<BTreeMap<String, serde_json::Value>>,
    redirect: Option<Vec<String>>,
}

impl fmt::Display for DataProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConversionError(msg) => {
                write!(f, "conversion from JSON to BrigadierTree failed: {msg}")
            }
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
            Self::ConversionError(..) => None,
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

fn to_brigadier_tree_node(
    name: &String,
    json_node: &BrigadierJsonNode,
    depth: u32,
) -> Result<BrigadierTreeNode, DataProviderError> {
    let children: BrigadierTreeNodeChildren = if let Some(json_children) = &json_node.children {
        BrigadierTreeNodeChildren::Nodes(
            json_children
                .iter()
                .map(|(name, child)| to_brigadier_tree_node(name, child, depth + 1))
                .collect::<Result<BTreeSet<BrigadierTreeNode>, DataProviderError>>()?,
        )
    } else if let Some(path) = &json_node.redirect {
        BrigadierTreeNodeChildren::Redirect(path.clone())
    } else {
        BrigadierTreeNodeChildren::Nodes(BTreeSet::new())
    };

    let name: String = name.clone();
    let executable: bool = json_node.executable.unwrap_or(false);
    match json_node.node_type {
        BrigadierJsonNodeType::Argument => match json_node.parser.clone() {
            Some(parser) => Ok(BrigadierTreeNode::Argument {
                name,
                executable,
                parser,
                properties: json_node.properties.clone(),
                children,
            }),
            None => Err(DataProviderError::ConversionError(String::from(
                "Found a `type=argument` node without a `parser`.",
            ))),
        },
        BrigadierJsonNodeType::Literal => Ok(BrigadierTreeNode::Literal {
            name,
            executable,
            children,
        }),
        BrigadierJsonNodeType::Root => Err(DataProviderError::ConversionError(String::from(
            "Found a `type=root` node within the JSON tree.",
        ))),
    }
}

impl TryFrom<BrigadierJsonNode> for BrigadierTree {
    type Error = DataProviderError;

    fn try_from(value: BrigadierJsonNode) -> Result<Self, Self::Error> {
        Ok(Self {
            commands: if let Some(ch) = &value.children {
                ch.iter()
                    .map(|(name, child)| to_brigadier_tree_node(name, child, 1))
                    .collect::<Result<BTreeSet<BrigadierTreeNode>, DataProviderError>>()?
            } else {
                BTreeSet::new()
            },
        })
    }
}
