use std::{env, error, process};

use brigadier_tree::BrigadierTree;
use code_generator::CodeGenerator;
use command_map::CommandMap;
use tree_provider::TreeProvider;

mod brigadier_tree;
mod code_generator;
mod command_map;
mod tree_provider;

fn exit_with_error(error: &dyn error::Error) -> ! {
    eprintln!("Error: {error}");
    process::exit(1)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let version: String = args[1].clone();

    let tree_provider: TreeProvider = TreeProvider::new();
    let tree: BrigadierTree = tree_provider
        .get_command_tree(version)
        .unwrap_or_else(|e| exit_with_error(&e));

    let commands: CommandMap = CommandMap::from(tree);

    let code_generator: CodeGenerator = CodeGenerator::new();
    code_generator
        .write_typescript(String::from("out/commands.ts"), &commands)
        .unwrap_or_else(|e| exit_with_error(&e));
}
