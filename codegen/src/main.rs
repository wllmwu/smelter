use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let res = reqwest::blocking::get(
        "https://raw.githubusercontent.com/misode/mcmeta/refs/heads/summary/commands/data.json",
    )?;
    let json: CommandNode = res.json()?;

    println!("Body:\n{:#?}", json);

    Ok(())
}
