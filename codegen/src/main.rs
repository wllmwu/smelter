use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum NodeType {
    Argument,
    Literal,
    Root,
}

#[derive(Debug, Deserialize)]
struct CommandNode {
    #[serde(rename = "type")]
    node_type: NodeType,
    children: Option<HashMap<String, CommandNode>>,
    executable: Option<bool>,
    parser: Option<String>,
    properties: Option<HashMap<String, serde_json::Value>>,
}

fn visit(node: &CommandNode, name: &str, path: &Vec<&str>) {
    let node_name = match node.node_type {
        NodeType::Argument => &format!("<{}>", name),
        NodeType::Literal => name,
        NodeType::Root => "",
    };
    if node.executable.is_some_and(|x| x) {
        for n in path {
            print!("{} ", n);
        }
        println!("{}", node_name)
    }
    if let Some(children) = &node.children {
        let mut child_names: Vec<&String> = children.keys().collect();
        child_names.sort();
        for child_name in child_names {
            let mut child_path = path.clone();
            if node.node_type != NodeType::Root {
                child_path.push(node_name);
            }
            visit(&children[child_name], child_name, &child_path);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let res = reqwest::blocking::get(
        "https://raw.githubusercontent.com/misode/mcmeta/refs/heads/summary/commands/data.json",
    )?;
    let json: CommandNode = res.json()?;

    visit(&json, "", &Vec::new());

    Ok(())
}
