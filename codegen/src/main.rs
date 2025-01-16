use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum BrigadierJsonNodeType {
    Argument,
    Literal,
    Root,
}

#[derive(Debug, Deserialize)]
struct BrigadierJsonNode {
    #[serde(rename = "type")]
    node_type: BrigadierJsonNodeType,
    children: Option<BTreeMap<String, BrigadierJsonNode>>,
    executable: Option<bool>,
    parser: Option<String>,
    properties: Option<BTreeMap<String, serde_json::Value>>,
    redirect: Option<Vec<String>>,
}

impl BrigadierJsonNode {
    fn is_leaf(&self) -> bool {
        self.children.is_none() && self.redirect.is_none()
    }
}

#[derive(Debug)]
enum BrigadierTreeNode {
    Alias {
        name: String,
        redirect: Vec<String>,
    },
    Argument {
        name: String,
        executable: bool,
        parser: String,
        properties: Option<BTreeMap<String, serde_json::Value>>,
        children: BTreeSet<BrigadierTreeNode>,
    },
    Literal {
        name: String,
        executable: bool,
        children: BTreeSet<BrigadierTreeNode>,
    },
}

impl BrigadierTreeNode {
    fn get_name(&self) -> &String {
        match self {
            BrigadierTreeNode::Alias { name, .. } => name,
            BrigadierTreeNode::Argument { name, .. } => name,
            BrigadierTreeNode::Literal { name, .. } => name,
        }
    }
}

impl PartialEq for BrigadierTreeNode {
    fn eq(&self, other: &Self) -> bool {
        self.get_name() == other.get_name()
    }
}

impl Eq for BrigadierTreeNode {}

impl PartialOrd for BrigadierTreeNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BrigadierTreeNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.get_name().cmp(other.get_name())
    }
}

#[derive(Debug)]
struct BrigadierTree {
    commands: BTreeSet<BrigadierTreeNode>,
}

fn to_brigadier_tree_node(
    name: &String,
    json_node: &BrigadierJsonNode,
    depth: u32,
) -> BrigadierTreeNode {
    if let Some(redirect) = &json_node.redirect {
        return BrigadierTreeNode::Alias {
            name: name.clone(),
            redirect: redirect.clone(),
        };
    }

    let children: BTreeSet<BrigadierTreeNode> = if let Some(json_children) = &json_node.children {
        let mut leaf_literals: BTreeSet<&String> = json_children
            .iter()
            .filter_map(|(name, child)| {
                if child.node_type == BrigadierJsonNodeType::Literal && child.is_leaf() {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();
        if depth <= 1 || leaf_literals.len() <= 1 {
            leaf_literals.clear();
        }

        let mut mapped_children: BTreeSet<BrigadierTreeNode> = json_children
            .iter()
            .filter_map(|(name, child)| {
                if !leaf_literals.contains(name) {
                    Some(to_brigadier_tree_node(name, child, depth + 1))
                } else {
                    None
                }
            })
            .collect();

        // Collapse sibling leaf literals
        if !leaf_literals.is_empty() {
            mapped_children.insert(BrigadierTreeNode::Argument {
                name: leaf_literals
                    .iter()
                    .cloned()
                    .cloned()
                    .collect::<Vec<String>>()
                    .join("|"),
                executable: true,
                parser: String::from("TODO"),
                properties: None,
                children: BTreeSet::new(),
            });
        }

        mapped_children
    } else {
        BTreeSet::new()
    };

    let name: String = name.clone();
    let executable: bool = json_node.executable.unwrap_or(false);
    match json_node.node_type {
        BrigadierJsonNodeType::Argument => BrigadierTreeNode::Argument {
            name,
            executable,
            parser: json_node
                .parser
                .clone()
                .expect("Argument node should have a parser"),
            properties: json_node.properties.clone(),
            children,
        },
        BrigadierJsonNodeType::Literal => BrigadierTreeNode::Literal {
            name,
            executable,
            children,
        },
        BrigadierJsonNodeType::Root => panic!("Found a `type=root` node within the JSON tree"),
    }
}

fn to_brigadier_tree(json: &BrigadierJsonNode) -> BrigadierTree {
    BrigadierTree {
        commands: if let Some(ch) = &json.children {
            ch.iter()
                .map(|(name, child)| to_brigadier_tree_node(name, child, 1))
                .collect()
        } else {
            BTreeSet::new()
        },
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let res = reqwest::blocking::get(
        "https://raw.githubusercontent.com/misode/mcmeta/refs/heads/summary/commands/data.json",
    )?;
    let json: BrigadierJsonNode = res.json()?;

    let tree: BrigadierTree = to_brigadier_tree(&json);
    println!("{:#?}", tree);

    Ok(())
}
