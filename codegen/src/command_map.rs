use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::brigadier_tree::{BrigadierTree, BrigadierTreeNode, BrigadierTreeNodeChildren};

/* * * * Public interface * * * */

pub enum CommandToken {
    Argument {
        name: String,
        parser: String,
        is_optional: bool,
    },
    Enum {
        values: Vec<String>,
        is_optional: bool,
    },
    Literal {
        value: String,
        is_optional: bool,
    },
}

pub type CommandMap = BTreeMap<String, Vec<Vec<CommandToken>>>;

impl From<BrigadierTree> for CommandMap {
    fn from(tree: BrigadierTree) -> Self {
        map_commands(handle_execute_command(consolidate_literals_into_enums(
            tree,
        )))
    }
}

/* * * * Private implementation * * * */

fn consolidate_literals_into_enums_inner(node: &BrigadierTreeNode) -> (BrigadierTreeNode, String) {
    let children: &BTreeSet<BrigadierTreeNode> = match node.get_children_or_redirect() {
        BrigadierTreeNodeChildren::Nodes(nodes) => nodes,
        BrigadierTreeNodeChildren::Redirect(path) => {
            return (
                node.clone(BrigadierTreeNodeChildren::Redirect(path.clone())),
                format!("->{}", path.join(",")),
            );
        }
    };
    let mut new_children_and_subtrees: BTreeSet<(BrigadierTreeNode, String)> = BTreeSet::new();
    let mut merge_candidates: HashMap<(String, bool), Vec<BrigadierTreeNode>> = HashMap::new();
    for child in children {
        let (new_child, new_child_subtrees_canon) = consolidate_literals_into_enums_inner(child);
        match new_child {
            BrigadierTreeNode::Literal { .. } => {
                let key: (String, bool) = (new_child_subtrees_canon, new_child.is_executable());
                if !merge_candidates.contains_key(&key) {
                    merge_candidates.insert(key.clone(), Vec::new());
                }
                merge_candidates.get_mut(&key).unwrap().push(new_child);
            }
            _ => {
                new_children_and_subtrees.insert((new_child, new_child_subtrees_canon));
            }
        }
    }
    for ((subtree_canon, is_executable), candidates) in merge_candidates.iter_mut() {
        let subtree_canon: String = subtree_canon.clone();
        if candidates.is_empty() {
            continue;
        } else if candidates.len() == 1 {
            new_children_and_subtrees.insert((candidates.pop().unwrap(), subtree_canon));
        } else {
            let enum_values: Vec<String> =
                candidates.iter().map(|n| n.get_name().clone()).collect();
            let enum_children: BrigadierTreeNodeChildren = match candidates.pop().unwrap() {
                BrigadierTreeNode::Literal { children, .. } => children,
                _ => panic!("Expected merge candidates to all be literals"),
            };
            new_children_and_subtrees.insert((
                BrigadierTreeNode::Enum {
                    name: enum_values.join("|"),
                    values: enum_values,
                    executable: is_executable.clone(),
                    children: enum_children,
                },
                subtree_canon,
            ));
        }
    }
    let mut new_children: BTreeSet<BrigadierTreeNode> = BTreeSet::new();
    let mut subtrees: Vec<String> = Vec::new();
    for (new_child, new_child_subtrees) in new_children_and_subtrees.into_iter() {
        let new_child_canon: String = new_child.get_canonical_string();
        new_children.insert(new_child);
        subtrees.push(format!("{}[{}]", new_child_canon, new_child_subtrees));
    }
    (
        node.clone(BrigadierTreeNodeChildren::Nodes(new_children)),
        subtrees.join(";"),
    )
}

fn consolidate_literals_into_enums(tree: BrigadierTree) -> BrigadierTree {
    BrigadierTree {
        commands: tree
            .commands
            .iter()
            .map(|node| consolidate_literals_into_enums_inner(node).0)
            .collect(),
    }
}

fn handle_execute_command_inner(node: &BrigadierTreeNode) -> BrigadierTreeNode {
    let new_children: BrigadierTreeNodeChildren = match node.get_children_or_redirect() {
        BrigadierTreeNodeChildren::Nodes(nodes) => BrigadierTreeNodeChildren::Nodes(
            nodes
                .iter()
                .map(|child| handle_execute_command_inner(child))
                .collect(),
        ),
        BrigadierTreeNodeChildren::Redirect(path) => {
            let mut new_nodes: BTreeSet<BrigadierTreeNode> = BTreeSet::new();
            if path.first().unwrap() == "execute" {
                new_nodes.insert(BrigadierTreeNode::Literal {
                    name: String::from("run"),
                    executable: false,
                    children: BrigadierTreeNodeChildren::Nodes(BTreeSet::new()),
                });
            }
            BrigadierTreeNodeChildren::Nodes(new_nodes)
        }
    };
    node.clone(new_children)
}

fn handle_execute_command(tree: BrigadierTree) -> BrigadierTree {
    BrigadierTree {
        commands: tree
            .commands
            .into_iter()
            .map(|command| {
                if command.get_name() == "execute" {
                    handle_execute_command_inner(&command)
                } else {
                    command
                }
            })
            .collect(),
    }
}

fn list_command_variants(
    node: &BrigadierTreeNode,
    branch: &Vec<&BrigadierTreeNode>,
    commands: &mut BTreeMap<String, Vec<Vec<CommandToken>>>,
) {
    let mut branch_copy: Vec<&BrigadierTreeNode> = branch.clone();
    branch_copy.push(node);
    let mut children_to_iterate: Option<&BTreeSet<BrigadierTreeNode>> = node.get_children();
    let command_name: &String = branch_copy.first().unwrap().get_name();

    if node.is_executable() || node.is_followed_by_any_command() {
        // Include consecutive descendants which are executable and single children as optional parameters
        let node_index: usize = branch_copy.len() - 1;
        let mut branch_last_node: &BrigadierTreeNode = node;
        loop {
            let children: &BTreeSet<BrigadierTreeNode> = match branch_last_node.get_children() {
                Some(nodes) => nodes,
                None => break,
            };
            if children.len() != 1 {
                break;
            }
            let optional_candidate: &BrigadierTreeNode = children.first().unwrap();
            if !optional_candidate.is_executable() {
                break;
            }
            branch_copy.push(optional_candidate);
            branch_last_node = optional_candidate;
        }
        children_to_iterate = branch_last_node.get_children();

        let mut command_tokens: Vec<CommandToken> = branch_copy[1..]
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let is_optional: bool = i + 1 > node_index;
                match n {
                    BrigadierTreeNode::Argument { name, parser, .. } => CommandToken::Argument {
                        name: name.clone(),
                        parser: parser.clone(),
                        is_optional,
                    },
                    BrigadierTreeNode::Enum { values, .. } => CommandToken::Enum {
                        values: values.clone(),
                        is_optional,
                    },
                    BrigadierTreeNode::Literal { name, .. } => CommandToken::Literal {
                        value: name.clone(),
                        is_optional,
                    },
                }
            })
            .collect();
        if branch_last_node.is_followed_by_any_command() {
            command_tokens.push(CommandToken::Argument {
                name: String::from("callback"),
                parser: String::from("TODO"),
                is_optional: false,
            });
        }

        let command_variants: &mut Vec<Vec<CommandToken>> = commands.get_mut(command_name).unwrap();
        command_variants.push(command_tokens);
    }

    if let Some(children) = children_to_iterate {
        for child in children {
            list_command_variants(child, &branch_copy, commands);
        }
    }
}

fn map_commands(tree: BrigadierTree) -> CommandMap {
    let mut commands: CommandMap = CommandMap::new();
    for node in &tree.commands {
        commands.insert(node.get_name().clone(), Vec::new());
        list_command_variants(&node, &Vec::new(), &mut commands);
        if commands.get(node.get_name()).unwrap().is_empty() {
            commands.remove(node.get_name());
        }
    }
    commands
}
