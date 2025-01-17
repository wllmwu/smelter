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

    fn is_executable(&self) -> bool {
        match self {
            BrigadierTreeNode::Alias { .. } => false,
            BrigadierTreeNode::Argument { executable, .. } => *executable,
            BrigadierTreeNode::Literal { executable, .. } => *executable,
        }
    }

    fn get_children(&self) -> Option<&BTreeSet<BrigadierTreeNode>> {
        match self {
            BrigadierTreeNode::Alias { .. } => None,
            BrigadierTreeNode::Argument { children, .. } => Some(children),
            BrigadierTreeNode::Literal { children, .. } => Some(children),
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

#[derive(Debug)]
enum CommandToken {
    Argument { name: String, data_type: String },
    Literal { value: String },
}

#[derive(Debug)]
struct NamedParameter {
    name: String,
    data_type: String,
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

fn to_brigadier_tree(json: BrigadierJsonNode) -> BrigadierTree {
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

fn enumerate_commands(
    node: &BrigadierTreeNode,
    branch: &Vec<&BrigadierTreeNode>,
    commands: &mut Vec<Vec<CommandToken>>,
) {
    if let BrigadierTreeNode::Alias { .. } = node {
        return;
    }
    let mut branch_copy: Vec<&BrigadierTreeNode> = branch.clone();
    branch_copy.push(node);
    if node.is_executable() {
        commands.push(
            branch_copy
                .iter()
                .filter_map(|n| match n {
                    BrigadierTreeNode::Alias { .. } => None,
                    BrigadierTreeNode::Argument { name, parser, .. } => {
                        Some(CommandToken::Argument {
                            name: name.clone(),
                            data_type: parser.clone(),
                        })
                    }
                    BrigadierTreeNode::Literal { name, .. } => Some(CommandToken::Literal {
                        value: name.clone(),
                    }),
                })
                .collect(),
        );
    }
    if let Some(children) = node.get_children() {
        for child in children {
            enumerate_commands(child, &branch_copy, commands);
        }
    }
}

fn to_commands(tree: BrigadierTree) -> Vec<Vec<CommandToken>> {
    let mut commands: Vec<Vec<CommandToken>> = Vec::new();
    for node in &tree.commands {
        enumerate_commands(&node, &Vec::new(), &mut commands);
    }
    commands
}

fn get_name_and_parameters(command: &Vec<CommandToken>) -> (Vec<String>, Vec<NamedParameter>) {
    let mut name: Vec<String> = Vec::new();
    let mut parameters: Vec<NamedParameter> = Vec::new();
    for token in command {
        match token {
            CommandToken::Argument { name, data_type } => parameters.push(NamedParameter {
                name: name.clone(),
                data_type: data_type.clone(),
            }),
            CommandToken::Literal { value } => name.push(value.clone()),
        }
    }
    (name, parameters)
}

fn to_function_signatures(
    commands: Vec<Vec<CommandToken>>,
) -> BTreeMap<Vec<String>, Vec<Vec<NamedParameter>>> {
    let mut signatures: BTreeMap<Vec<String>, Vec<Vec<NamedParameter>>> = BTreeMap::new();
    for command in &commands {
        let (name, parameters) = get_name_and_parameters(command);
        if let Some(signature) = signatures.get_mut(&name) {
            signature.push(parameters);
        } else {
            signatures.insert(name, vec![parameters]);
        }
    }
    signatures
}

fn emit_function_signatures_typescript(
    signatures: &BTreeMap<Vec<String>, Vec<Vec<NamedParameter>>>,
) {
    for (name, parameters) in signatures {
        let upper_snake_case_name: Vec<String> = name
            .iter()
            .map(|s| s[0..1].to_ascii_uppercase() + &s[1..])
            .collect();
        let args_type_name: String = format!("{}Args", upper_snake_case_name.join(""));
        let function_name: String = format!("{}{}", name[0], upper_snake_case_name[1..].join(""));
        println!("type {} =", args_type_name);
        for (i, parameter_combination) in parameters.iter().enumerate() {
            let object_type_properties: Vec<String> = parameter_combination
                .iter()
                .map(|NamedParameter { name, data_type }| format!("{}: \"{}\"", name, data_type))
                .collect();
            print!("  | {{ {} }}", object_type_properties.join(", "));
            if i < parameters.len() {
                println!();
            } else {
                println!(";");
            }
        }
        println!("function {}(args: {}) {{}}", function_name, args_type_name);
        println!();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let res = reqwest::blocking::get(
        "https://raw.githubusercontent.com/misode/mcmeta/refs/heads/summary/commands/data.json",
    )?;
    let json: BrigadierJsonNode = res.json()?;

    let tree: BrigadierTree = to_brigadier_tree(json);

    let commands: Vec<Vec<CommandToken>> = to_commands(tree);
    // for command in &commands {
    //     println!(
    //         "{}",
    //         command
    //             .iter()
    //             .map(|token| match token {
    //                 CommandToken::Argument { name, .. } => format!("<{}>", name),
    //                 CommandToken::Literal { value } => value.clone(),
    //             })
    //             .collect::<Vec<String>>()
    //             .join(" ")
    //     )
    // }

    let generated_signatures = to_function_signatures(commands);
    emit_function_signatures_typescript(&generated_signatures);

    Ok(())
}
