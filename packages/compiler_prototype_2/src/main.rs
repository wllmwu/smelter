use anyhow::{Context, Result, bail};
use clap::Parser as CliParser;
use oxc::{
    allocator::Allocator,
    ast::ast::{Program, Statement},
    parser::Parser,
    semantic::SemanticBuilder,
    span::SourceType,
};

#[derive(CliParser)]
struct CliArguments {
    path: std::path::PathBuf,
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

    // println!("{:#?}", &program);

    let data_pack = compile_program(program)?;
    std::fs::create_dir_all("smelter_prototype/data/smelter/function")
        .with_context(|| "Couldn't create directories")?;
    for function in data_pack {
        std::fs::write(
            format!(
                "smelter_prototype/data/smelter/function/{}.mcfunction",
                &function.name
            ),
            function.body.join("\n"),
        )
        .with_context(|| format!("Couldn't write file `{}.mcfunction`", &function.name))?
    }
    std::fs::write(
        "smelter_prototype/pack.mcmeta",
        "{\"pack\":{\"description\":\"smelter prototype\",\"min_format\":[94,1],\"max_format\":[94,1]}}",
    ).with_context(|| "Couldn't write file: `pack.mcmeta`")?;

    Ok(())
}

struct Mcfunction {
    name: String,
    body: Vec<String>,
}

type DataPack = Vec<Mcfunction>;

struct DataPackBuilder {
    committed_mcfunctions: Vec<Mcfunction>,
    compiling_stack: Vec<Mcfunction>,
}

impl DataPackBuilder {
    fn new(system_mcfunctions: Vec<Mcfunction>) -> DataPackBuilder {
        DataPackBuilder {
            committed_mcfunctions: system_mcfunctions,
            compiling_stack: Vec::new(),
        }
    }

    fn push_mcfunction(&mut self, name: String) -> &mut Self {
        self.compiling_stack.push(Mcfunction {
            name,
            body: Vec::new(),
        });
        self
    }

    fn commit_mcfunction(&mut self) -> &mut Self {
        if let Some(mcf) = self.compiling_stack.pop() {
            self.committed_mcfunctions.push(mcf);
        }
        self
    }

    fn push_command(&mut self, command: String) -> &mut Self {
        if let Some(mcf) = self.compiling_stack.first_mut() {
            mcf.body.push(command);
        }
        self
    }

    fn extend_commands(&mut self, commands: Vec<String>) -> &mut Self {
        if let Some(mcf) = self.compiling_stack.first_mut() {
            mcf.body.extend(commands);
        }
        self
    }

    fn complete(self) -> DataPack {
        self.committed_mcfunctions
    }
}

fn debug_log(message: String) -> String {
    format!(
        "execute if score #debug smelter_internal matches 1.. run tellraw @a '[smelter] {message}'"
    )
}

fn debug_log_macro(message: String) -> String {
    format!("${}", debug_log(message))
}

fn compile_program(program: Program) -> Result<DataPack> {
    let mut builder = DataPackBuilder::new(vec![Mcfunction {
        name: String::from("initialize"),
        body: vec![
            String::from("data modify storage smelter:smelter heap set value []"),
            String::from("data modify storage smelter:smelter queue set value []"),
            String::from("data modify storage smelter:smelter stack set value []"),
        ],
    }]);

    builder
        .push_mcfunction(String::from("main"))
        .push_command(debug_log(String::from("entering main")))
        .push_command(String::from("function smelter:initialize"));

    for statement in &program.body {
        compile_statement(&mut builder, statement).with_context(|| "Couldn't compile program")?;
    }

    builder
        .push_command(debug_log(String::from("exiting main")))
        .commit_mcfunction();

    Ok(builder.complete())
}

fn compile_statement(builder: &mut DataPackBuilder, statement: &Statement) -> Result<()> {
    match statement {
        // Basics
        Statement::BlockStatement(block_statement) => todo!(),
        Statement::EmptyStatement(_) => (),
        Statement::ExpressionStatement(expression_statement) => todo!(),
        // Control flow
        Statement::BreakStatement(break_statement) => todo!(),
        Statement::ContinueStatement(continue_statement) => todo!(),
        Statement::DebuggerStatement(_) => bail!("Not supported: debugger statements"),
        Statement::DoWhileStatement(do_while_statement) => todo!(),
        Statement::ForInStatement(for_in_statement) => todo!(),
        Statement::ForOfStatement(for_of_statement) => todo!(),
        Statement::ForStatement(for_statement) => todo!(),
        Statement::IfStatement(if_statement) => todo!(),
        Statement::LabeledStatement(_) => bail!("Not supported: labeled statements"),
        Statement::ReturnStatement(return_statement) => todo!(),
        Statement::SwitchStatement(switch_statement) => todo!(),
        Statement::ThrowStatement(throw_statement) => todo!(),
        Statement::TryStatement(try_statement) => todo!(),
        Statement::WhileStatement(while_statement) => todo!(),
        Statement::WithStatement(_) => bail!("Not supported: with statements"),
        // Declarations
        Statement::ClassDeclaration(class_declaration) => todo!(),
        Statement::FunctionDeclaration(function_declaration) => todo!(),
        Statement::VariableDeclaration(variable_declaration) => todo!(),
        // Imports and exports
        Statement::ImportDeclaration(_) => bail!("Not supported: imports and exports"),
        Statement::ExportAllDeclaration(_) => bail!("Not supported: imports and exports"),
        Statement::ExportDefaultDeclaration(_) => bail!("Not supported: imports and exports"),
        Statement::ExportNamedDeclaration(_) => bail!("Not supported: imports and exports"),
        // TypeScript
        Statement::TSEnumDeclaration(_) => (),
        Statement::TSExportAssignment(_) => (),
        Statement::TSGlobalDeclaration(_) => (),
        Statement::TSImportEqualsDeclaration(_) => (),
        Statement::TSInterfaceDeclaration(_) => (),
        Statement::TSModuleDeclaration(_) => (),
        Statement::TSNamespaceExportDeclaration(_) => (),
        Statement::TSTypeAliasDeclaration(_) => (),
    }
    Ok(())
}
