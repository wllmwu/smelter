use std::{io, ops::RangeFrom};

use crate::command_map::{CommandMap, CommandToken};

pub(in crate::code_generator) fn write_to_typescript(
    commands: &CommandMap,
    writer: &mut dyn io::Write,
) -> Result<(), io::Error> {
    writeln!(writer, "type MinecraftCommands = {{")?;
    for (command_name, variants) in commands {
        writeln!(writer, "  {}: {{", fix_object_type_key(command_name))?;
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
            writeln!(writer, "    ({}): void;", parameters.join(", "))?;
        }
        writeln!(writer, "  }};")?;
    }
    writeln!(writer, "}};")?;
    writeln!(writer)?;

    writeln!(writer, "function __emitMacro<Command extends keyof MinecraftCommands>(command: Command): MinecraftCommands[typeof command] {{")?;
    writeln!(
        writer,
        "  return (...tokens: any[]) => console.log(\"$\", ...tokens);"
    )?;
    writeln!(writer, "}}")?;
    writeln!(writer)?;

    for command_name in commands.keys() {
        writeln!(
            writer,
            "export const {} = __emitMacro(\"{}\");",
            fix_identifier(command_name),
            command_name
        )?;
    }

    Ok(())
}

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
