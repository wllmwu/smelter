use std::{
    collections::{BTreeMap, BTreeSet},
    error, fmt,
};

use serde::Deserialize;

use crate::brigadier_tree::{BrigadierTree, BrigadierTreeNode, BrigadierTreeNodeChildren};

/* * * * Public interface * * * */

#[derive(Debug)]
pub enum ConversionError {
    DeserializationError(serde_json::Error),
    MalformedTree(String),
}

#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum BrigadierJsonNodeType {
    Argument,
    Literal,
    Root,
}

#[derive(Deserialize)]
pub(super) struct BrigadierJsonNode {
    #[serde(rename = "type")]
    node_type: BrigadierJsonNodeType,
    children: Option<BTreeMap<String, BrigadierJsonNode>>,
    executable: Option<bool>,
    parser: Option<String>,
    properties: Option<BTreeMap<String, serde_json::Value>>,
    redirect: Option<Vec<String>>,
}

impl TryFrom<String> for BrigadierJsonNode {
    type Error = ConversionError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(serde_json::from_str(&value)?)
    }
}

impl TryFrom<BrigadierJsonNode> for BrigadierTree {
    type Error = ConversionError;

    fn try_from(value: BrigadierJsonNode) -> Result<Self, Self::Error> {
        Ok(Self {
            commands: if let Some(ch) = &value.children {
                ch.iter()
                    .map(|(name, child)| to_brigadier_tree_node(name, child, 1))
                    .collect::<Result<BTreeSet<BrigadierTreeNode>, ConversionError>>()?
            } else {
                BTreeSet::new()
            },
        })
    }
}

/* * * * Private implementation * * * */

fn to_brigadier_tree_node(
    name: &String,
    json_node: &BrigadierJsonNode,
    depth: u32,
) -> Result<BrigadierTreeNode, ConversionError> {
    let children: BrigadierTreeNodeChildren = if let Some(json_children) = &json_node.children {
        BrigadierTreeNodeChildren::Nodes(
            json_children
                .iter()
                .map(|(name, child)| to_brigadier_tree_node(name, child, depth + 1))
                .collect::<Result<BTreeSet<BrigadierTreeNode>, ConversionError>>()?,
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
            None => Err(ConversionError::MalformedTree(String::from(
                "Found a `type=argument` node without a `parser`.",
            ))),
        },
        BrigadierJsonNodeType::Literal => Ok(BrigadierTreeNode::Literal {
            name,
            executable,
            children,
        }),
        BrigadierJsonNodeType::Root => Err(ConversionError::MalformedTree(String::from(
            "Found a `type=root` node within the JSON tree.",
        ))),
    }
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeserializationError(e) => {
                write!(f, "failed to deserialize from JSON: {e}")
            }
            Self::MalformedTree(msg) => {
                write!(f, "malformed tree: {msg}")
            }
        }
    }
}

impl error::Error for ConversionError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::DeserializationError(e) => Some(e),
            Self::MalformedTree(..) => None,
        }
    }
}

impl From<serde_json::Error> for ConversionError {
    fn from(value: serde_json::Error) -> Self {
        Self::DeserializationError(value)
    }
}
