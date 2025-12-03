use anyhow::{Context, Result};
use clap::Parser as CliParser;
use oxc::{
    allocator::Allocator,
    ast::ast::{Argument, Expression, Function, Program, Statement},
    parser::Parser,
    semantic::SemanticBuilder,
    span::SourceType,
};

#[derive(CliParser)]
struct CliArguments {
    path: std::path::PathBuf,
}

#[derive(Debug)]
enum SmelterExpression {
    ExecuteCommand { tail: String },
}

#[derive(Debug)]
struct SmelterFunction {
    body: Vec<SmelterExpression>,
}

#[derive(Debug)]
struct SmelterProgram {
    body: Vec<SmelterFunction>,
}

fn convert_program(program: &Program) -> SmelterProgram {
    SmelterProgram {
        body: program
            .body
            .iter()
            .filter_map(|statement| match statement {
                Statement::FunctionDeclaration(function_stmt) => {
                    Some(convert_function(function_stmt))
                }
                _ => None,
            })
            .collect::<Vec<SmelterFunction>>(),
    }
}

fn convert_function(function: &Function) -> SmelterFunction {
    if let Some(body) = &function.body {
        SmelterFunction {
            body: body
                .statements
                .iter()
                .filter_map(|statement| convert_statement(statement))
                .collect::<Vec<SmelterExpression>>(),
        }
    } else {
        SmelterFunction { body: Vec::new() }
    }
}

fn convert_statement(statement: &Statement) -> Option<SmelterExpression> {
    match statement {
        Statement::ExpressionStatement(expression_stmt) => {
            convert_expression(&expression_stmt.expression)
        }
        _ => None,
    }
}

fn convert_expression(expression: &Expression) -> Option<SmelterExpression> {
    match expression {
        Expression::CallExpression(call_expr) => match &call_expr.callee {
            Expression::Identifier(identifier) => {
                if identifier.name.as_str().to_string() == "execute" {
                    match &call_expr.arguments[0] {
                        Argument::StringLiteral(string_literal) => {
                            let tail = string_literal.value.as_str().to_string();
                            Some(SmelterExpression::ExecuteCommand { tail })
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        },
        _ => None,
    }
}

#[derive(Debug)]
enum DataPackFile {
    Mcfunction { contents: String },
}

#[derive(Debug)]
struct DataPack {
    files: Vec<DataPackFile>,
}

fn convert_mcfunction(function: &SmelterFunction) -> DataPackFile {
    DataPackFile::Mcfunction {
        contents: function
            .body
            .iter()
            .map(|expression| match expression {
                SmelterExpression::ExecuteCommand { tail } => format!("execute {tail}"),
            })
            .collect::<Vec<String>>()
            .join("\n"),
    }
}

fn convert_data_pack(program: &SmelterProgram) -> DataPack {
    DataPack {
        files: program
            .body
            .iter()
            .map(|function| convert_mcfunction(function))
            .collect::<Vec<DataPackFile>>(),
    }
}

fn main() -> Result<()> {
    let args = CliArguments::parse();
    let path = args.path;

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Couldn't read file `{}`", path.display()))?;

    let source_type =
        SourceType::from_path(path).with_context(|| format!("Couldn't identify source type"))?;
    let allocator = Allocator::default();
    let parser_result = Parser::new(&allocator, &content, source_type).parse();
    let program = parser_result.program;

    if !parser_result.errors.is_empty() {
        let error_messages = parser_result
            .errors
            .into_iter()
            .map(|error| format!("{:?}", error.with_source_code(content.clone())))
            .collect::<Vec<String>>()
            .join("\n");
        println!("Parse errors:\n{error_messages}");
    }

    let semantic_result = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&program);

    if !semantic_result.errors.is_empty() {
        let error_messages = semantic_result
            .errors
            .into_iter()
            .map(|error| format!("{:?}", error.with_source_code(content.clone())))
            .collect::<Vec<String>>()
            .join("\n");
        println!("Semantic errors:\n{error_messages}");
    }

    let smelter_program = convert_program(&program);

    let data_pack = convert_data_pack(&smelter_program);

    for (index, file) in data_pack.files.iter().enumerate() {
        let path = format!("out{index}.mcfunction");
        let contents = match file {
            DataPackFile::Mcfunction { contents } => contents,
        };
        std::fs::write(&path, contents)
            .with_context(|| format!("Couldn't write file `{}`", &path))?;
    }
    println!("Done");

    Ok(())
}
