use std::collections::{BTreeMap, BTreeSet};

/* * * * Public interface * * * */

pub enum BrigadierTreeNodeChildren {
    Nodes(BTreeSet<BrigadierTreeNode>),
    Redirect(Vec<String>),
}

pub enum BrigadierTreeNode {
    Argument {
        name: String,
        executable: bool,
        parser: String,
        properties: Option<BTreeMap<String, serde_json::Value>>,
        children: BrigadierTreeNodeChildren,
    },
    Enum {
        /** Only used for sorting nodes alphabetically. */
        name: String,
        values: Vec<String>,
        executable: bool,
        children: BrigadierTreeNodeChildren,
    },
    Literal {
        name: String,
        executable: bool,
        children: BrigadierTreeNodeChildren,
    },
}

pub struct BrigadierTree {
    pub commands: BTreeSet<BrigadierTreeNode>,
}

impl BrigadierTreeNode {
    pub fn get_name(&self) -> &String {
        match self {
            BrigadierTreeNode::Argument { name, .. } => name,
            BrigadierTreeNode::Enum { name, .. } => name,
            BrigadierTreeNode::Literal { name, .. } => name,
        }
    }

    pub fn is_executable(&self) -> bool {
        match self {
            BrigadierTreeNode::Argument { executable, .. } => *executable,
            BrigadierTreeNode::Enum { executable, .. } => *executable,
            BrigadierTreeNode::Literal { executable, .. } => *executable,
        }
    }

    pub fn get_canonical_string(&self) -> String {
        match self {
            BrigadierTreeNode::Argument {
                name,
                executable,
                parser,
                ..
            } => format!("Arg({},{},{})", name, parser, executable),
            BrigadierTreeNode::Enum {
                values, executable, ..
            } => format!("En({},{})", values.join("|"), executable),
            BrigadierTreeNode::Literal {
                name, executable, ..
            } => format!("Lit({},{})", name, executable),
        }
    }

    pub fn get_children_or_redirect(&self) -> &BrigadierTreeNodeChildren {
        match self {
            BrigadierTreeNode::Argument { children, .. } => children,
            BrigadierTreeNode::Enum { children, .. } => children,
            BrigadierTreeNode::Literal { children, .. } => children,
        }
    }

    pub fn get_children(&self) -> Option<&BTreeSet<BrigadierTreeNode>> {
        match self.get_children_or_redirect() {
            BrigadierTreeNodeChildren::Nodes(nodes) => Some(nodes),
            BrigadierTreeNodeChildren::Redirect(..) => None,
        }
    }

    pub fn clone(&self, children: BrigadierTreeNodeChildren) -> Self {
        match self {
            BrigadierTreeNode::Argument {
                name,
                executable,
                parser,
                properties,
                ..
            } => BrigadierTreeNode::Argument {
                name: name.clone(),
                executable: executable.clone(),
                parser: parser.clone(),
                properties: properties.clone(),
                children,
            },
            BrigadierTreeNode::Enum {
                name,
                values,
                executable,
                ..
            } => BrigadierTreeNode::Enum {
                name: name.clone(),
                values: values.clone(),
                executable: executable.clone(),
                children,
            },
            BrigadierTreeNode::Literal {
                name, executable, ..
            } => BrigadierTreeNode::Literal {
                name: name.clone(),
                executable: executable.clone(),
                children,
            },
        }
    }

    pub fn is_followed_by_any_command(&self) -> bool {
        !self.is_executable()
            && match self.get_children_or_redirect() {
                BrigadierTreeNodeChildren::Nodes(nodes) => nodes.is_empty(),
                BrigadierTreeNodeChildren::Redirect(..) => false,
            }
    }
}

/* * * * Private implementation * * * */

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
