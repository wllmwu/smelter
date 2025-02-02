use std::env;

use brigadier_tree::BrigadierTree;
use code_generator::CodeGenerator;
use command_map::CommandMap;
use tree_provider::TreeProvider;

mod brigadier_tree;
mod code_generator;
mod command_map;
mod tree_provider;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let version: String = args[1].clone();

    let tree_provider: TreeProvider = TreeProvider::new();
    let tree: BrigadierTree = tree_provider.get_command_tree(version)?;

    let commands: CommandMap = CommandMap::from(tree);

    let code_generator: CodeGenerator = CodeGenerator::new();
    code_generator.write_typescript(String::from("out/commands.ts"), &commands)?;

    Ok(())
}
