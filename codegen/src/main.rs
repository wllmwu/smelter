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
                name: String::from("option"),
                executable: true,
                parser: leaf_literals
                    .iter()
                    .cloned()
                    .cloned()
                    .collect::<Vec<String>>()
                    .join("|"),
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

fn list_command_variants(
    node: &BrigadierTreeNode,
    branch: &Vec<&BrigadierTreeNode>,
    commands: &mut BTreeMap<String, Vec<Vec<CommandToken>>>,
) {
    if let BrigadierTreeNode::Alias { .. } = node {
        return;
    }
    let mut branch_copy: Vec<&BrigadierTreeNode> = branch.clone();
    branch_copy.push(node);
    if node.is_executable() {
        let command_variants = commands
            .get_mut(branch_copy.first().unwrap().get_name())
            .unwrap();
        command_variants.push(
            branch_copy[1..]
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
            let parameters: Vec<String> = tokens
                .iter()
                .enumerate()
                .map(|(i, token)| match token {
                    CommandToken::Argument { name, data_type } => {
                        format!("{}: \"{}\"", fix_identifier(name), data_type)
                    }
                    CommandToken::Literal { value } => {
                        format!("{}: \"{}\"", String::from("lit") + &i.to_string(), value)
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

    let tree: BrigadierTree = to_brigadier_tree(json);

    let commands: BTreeMap<String, Vec<Vec<CommandToken>>> = to_commands(tree);

    emit_generated_typescript(&commands);

    Ok(())
}
