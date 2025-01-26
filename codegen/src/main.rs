use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    ops::RangeFrom,
};

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

#[derive(Debug)]
enum BrigadierTreeNodeChildren {
    Nodes(BTreeSet<BrigadierTreeNode>),
    Redirect(Vec<String>),
}

#[derive(Debug)]
enum BrigadierTreeNode {
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

impl BrigadierTreeNode {
    fn get_name(&self) -> &String {
        match self {
            BrigadierTreeNode::Argument { name, .. } => name,
            BrigadierTreeNode::Enum { name, .. } => name,
            BrigadierTreeNode::Literal { name, .. } => name,
        }
    }

    fn is_executable(&self) -> bool {
        match self {
            BrigadierTreeNode::Argument { executable, .. } => *executable,
            BrigadierTreeNode::Enum { executable, .. } => *executable,
            BrigadierTreeNode::Literal { executable, .. } => *executable,
        }
    }

    fn get_canonical_string(&self) -> String {
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

    fn get_children_or_redirect(&self) -> &BrigadierTreeNodeChildren {
        match self {
            BrigadierTreeNode::Argument { children, .. } => children,
            BrigadierTreeNode::Enum { children, .. } => children,
            BrigadierTreeNode::Literal { children, .. } => children,
        }
    }

    fn get_children(&self) -> Option<&BTreeSet<BrigadierTreeNode>> {
        match self.get_children_or_redirect() {
            BrigadierTreeNodeChildren::Nodes(nodes) => Some(nodes),
            BrigadierTreeNodeChildren::Redirect(..) => None,
        }
    }

    fn clone(&self, children: BrigadierTreeNodeChildren) -> Self {
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

    fn is_followed_by_any_command(&self) -> bool {
        !self.is_executable()
            && match self.get_children_or_redirect() {
                BrigadierTreeNodeChildren::Nodes(nodes) => nodes.is_empty(),
                BrigadierTreeNodeChildren::Redirect(..) => false,
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
    Argument {
        name: String,
        data_type: String,
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

fn to_brigadier_tree_node(
    name: &String,
    json_node: &BrigadierJsonNode,
    depth: u32,
) -> BrigadierTreeNode {
    let children: BrigadierTreeNodeChildren = if let Some(json_children) = &json_node.children {
        BrigadierTreeNodeChildren::Nodes(
            json_children
                .iter()
                .map(|(name, child)| to_brigadier_tree_node(name, child, depth + 1))
                .collect(),
        )
    } else if let Some(path) = &json_node.redirect {
        BrigadierTreeNodeChildren::Redirect(path.clone())
    } else {
        BrigadierTreeNodeChildren::Nodes(BTreeSet::new())
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
                let key = (new_child_subtrees_canon, new_child.is_executable());
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
        let new_child_canon = new_child.get_canonical_string();
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
                        data_type: parser.clone(),
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
                data_type: String::from("TODO"),
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

fn to_commands(tree: BrigadierTree) -> BTreeMap<String, Vec<Vec<CommandToken>>> {
    let mut commands: BTreeMap<String, Vec<Vec<CommandToken>>> = BTreeMap::new();
    for node in &tree.commands {
        commands.insert(node.get_name().clone(), Vec::new());
        list_command_variants(&node, &Vec::new(), &mut commands);
        if commands.get(node.get_name()).unwrap().is_empty() {
            commands.remove(node.get_name());
        }
    }
    commands
}

// Static string array: https://stackoverflow.com/a/32383866
static RESERVED_KEYWORDS_JAVASCRIPT: &[&str] = &[
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "null",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "let",
    "static",
    "yield",
    "await",
    "enum",
    "implements",
    "interface",
    "package",
    "private",
    "protected",
    "public",
    "arguments",
    "eval",
];

fn fix_identifier(identifier: &String) -> String {
    let without_dashes: String = identifier
        .clone()
        .split("-")
        .enumerate()
        .map(|(i, s)| {
            if i == 0 {
                s.to_string()
            } else {
                s[0..1].to_ascii_uppercase() + &s[1..]
            }
        })
        .collect::<Vec<String>>()
        .join("");
    for keyword in RESERVED_KEYWORDS_JAVASCRIPT {
        if without_dashes.eq(keyword) {
            return without_dashes + "_";
        }
    }
    without_dashes
}

fn fix_object_type_key(key: &String) -> String {
    if key.contains("-") {
        format!("\"{}\"", key)
    } else {
        key.clone()
    }
}

fn emit_generated_typescript(commands: &BTreeMap<String, Vec<Vec<CommandToken>>>) {
    println!("type MinecraftCommands = {{");
    for (command_name, variants) in commands {
        println!("  {}: {{", fix_object_type_key(command_name));
        for tokens in variants {
            let mut literal_index: RangeFrom<i32> = 0..;
            let parameters: Vec<String> = tokens
                .iter()
                .map(|token| match token {
                    CommandToken::Argument {
                        name,
                        data_type,
                        is_optional,
                    } => {
                        format!(
                            "{}{}: \"{}\"",
                            fix_identifier(name),
                            if *is_optional { "?" } else { "" },
                            data_type
                        )
                    }
                    CommandToken::Enum {
                        values,
                        is_optional,
                    } => {
                        format!(
                            "{}{}: {}",
                            String::from("opt") + &literal_index.next().unwrap().to_string(),
                            if *is_optional { "?" } else { "" },
                            values
                                .iter()
                                .map(|v| format!("\"{}\"", v))
                                .collect::<Vec<String>>()
                                .join(" | ")
                        )
                    }
                    CommandToken::Literal { value, is_optional } => {
                        format!(
                            "{}{}: \"{}\"",
                            String::from("opt") + &literal_index.next().unwrap().to_string(),
                            if *is_optional { "?" } else { "" },
                            value
                        )
                    }
                })
                .collect();
            println!("    ({}): void;", parameters.join(", "));
        }
        println!("  }};");
    }
    println!("}};");
    println!();

    println!("function __emitMacro<Command extends keyof MinecraftCommands>(command: Command): MinecraftCommands[typeof command] {{");
    println!("  return (...tokens: any[]) => console.log(\"$\", ...tokens);");
    println!("}}");
    println!();

    for command_name in commands.keys() {
        println!(
            "export const {} = __emitMacro(\"{}\");",
            fix_identifier(command_name),
            command_name
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let res = reqwest::blocking::get(
        "https://raw.githubusercontent.com/misode/mcmeta/refs/heads/summary/commands/data.json",
    )?;
    let json: BrigadierJsonNode = res.json()?;

    let tree: BrigadierTree =
        handle_execute_command(consolidate_literals_into_enums(to_brigadier_tree(json)));

    let commands: BTreeMap<String, Vec<Vec<CommandToken>>> = to_commands(tree);

    emit_generated_typescript(&commands);

    Ok(())
}
