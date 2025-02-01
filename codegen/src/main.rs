use std::{collections::BTreeMap, env, ops::RangeFrom};

use brigadier_tree::BrigadierTree;
use command_map::{CommandMap, CommandToken};
use tree_provider::TreeProvider;

mod brigadier_tree;
mod command_map;
mod tree_provider;

// Static string array: https://stackoverflow.com/a/32383866
// JavaScript reserved words: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Lexical_grammar#reserved_words
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
                        parser,
                        is_optional,
                    } => {
                        format!(
                            "{}{}: \"{}\"",
                            fix_identifier(&name),
                            if *is_optional { "?" } else { "" },
                            parser
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

/**
 * args: version (or data file?), language(s?)
 * 1. check if command data for specified version is already cached
 * 2. if not, then download it from mcmeta repo
 *    1. use github api to find commit in summary branch with version number in its message: https://docs.github.com/en/rest/commits/commits?apiVersion=2022-11-28#list-commits
 *    2. download raw json file: https://raw.githubusercontent.com/misode/mcmeta/<commitsha>/commands/data.json
 *    3. write file to local (gitignored) cache
 * 3. construct brigadiertree from cached json for specified version
 * 4. convert brigadiertree into map of command name to command variants (including optimizations)
 * 5. write generated api for specified language to output directory
 * after: manually review generated api, add missing arg type definitions + remove obsolete ones
 */
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let version: String = args[1].clone();

    let tree_provider: TreeProvider = TreeProvider::new();
    let tree: BrigadierTree = tree_provider.get_command_tree(version)?;

    let commands: CommandMap = CommandMap::from(tree);

    emit_generated_typescript(&commands);

    Ok(())
}
