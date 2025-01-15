use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum NodeType {
    Argument,
    Literal,
    Root,
}

#[derive(Debug, Deserialize)]
struct BrigadierCommandNode {
    #[serde(rename = "type")]
    node_type: NodeType,
    children: Option<BTreeMap<String, BrigadierCommandNode>>,
    executable: Option<bool>,
    parser: Option<String>,
    properties: Option<BTreeMap<String, serde_json::Value>>,
    redirect: Option<Vec<String>>,
}

impl BrigadierCommandNode {
    fn is_leaf(&self) -> bool {
        self.children.is_none() && self.redirect.is_none()
    }
}

fn print_tree(node: &BrigadierCommandNode, name: &str, path: &Vec<&str>) {
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
        for child_name in children.keys() {
            let mut child_path = path.clone();
            if node.node_type != NodeType::Root {
                child_path.push(node_name);
            }
            print_tree(&children[child_name], child_name, &child_path);
        }
    }
}

fn collapse_leaf_literals(node: &mut BrigadierCommandNode, depth: u32) {
    if let Some(children) = &mut node.children {
        if depth > 1 {
            let mut literals: Vec<String> = Vec::new();
            let names: Vec<String> = children.keys().cloned().collect();
            for name in names {
                let child = children.get(&name).expect("Map should have value for key");
                if child.node_type == NodeType::Literal && child.is_leaf() {
                    literals.push(name);
                }
            }
            if literals.len() > 1 {
                for name in literals.iter() {
                    children.remove(name);
                }
                children.insert(
                    literals.join("|"),
                    BrigadierCommandNode {
                        node_type: NodeType::Argument,
                        children: None,
                        executable: Some(true),
                        parser: Some(String::from("todo")),
                        properties: None,
                        redirect: None,
                    },
                );
            }
        }
        for child in children.values_mut() {
            collapse_leaf_literals(child, depth + 1);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let res = reqwest::blocking::get(
        "https://raw.githubusercontent.com/misode/mcmeta/refs/heads/summary/commands/data.json",
    )?;
    let mut json: BrigadierCommandNode = res.json()?;

    // Algorithm:
    // 1. Collapse sibling leaf literals: JsonCommandNode -> JsonCommandNode
    // 2. Split tree into forest by literals: JsonCommandNode -> FunctionNode(ParameterNode)[]
    // 3. Traverse parameter trees and generate one parameter object type for each executable node: FunctionNode[] -> FunctionSignature(NamedParameter[])[]
    // 4. Emit TypeScript type definitions and function signatures: FunctionSignature[] -> String[]

    collapse_leaf_literals(&mut json, 0);
    print_tree(&json, "", &Vec::new());

    Ok(())
}
