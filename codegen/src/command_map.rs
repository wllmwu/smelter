use std::collections::{BTreeMap, BTreeSet};

use crate::brigadier_tree::{BrigadierTree, BrigadierTreeNode};

mod preprocessors;

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

pub type CommandVariant = Vec<CommandToken>;
pub type CommandMap = BTreeMap<String, Vec<CommandVariant>>;

impl From<BrigadierTree> for CommandMap {
    fn from(tree: BrigadierTree) -> Self {
        map_commands(preprocessors::handle_execute_command(tree))
    }
}

/* * * * Private implementation * * * */

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

fn list_command_variants(
    node: &BrigadierTreeNode,
    branch: &Vec<&BrigadierTreeNode>,
    commands: &mut CommandMap,
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

        let mut command_tokens: CommandVariant = branch_copy[1..]
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

        let command_variants: &mut Vec<CommandVariant> = commands.get_mut(command_name).unwrap();
        command_variants.push(command_tokens);
    }

    if let Some(children) = children_to_iterate {
        for child in children {
            list_command_variants(child, &branch_copy, commands);
        }
    }
}
