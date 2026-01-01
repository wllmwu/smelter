use anyhow::{Context, Result, bail};
use clap::Parser as CliParser;
use oxc::{
    allocator::Allocator,
    ast::ast::{Expression, Program, Statement},
    parser::Parser,
    semantic::SemanticBuilder,
    span::{SourceType, Span},
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

impl Mcfunction {
    fn new_static(name: &str, parameters: &[&str], body: &[&str]) -> Mcfunction {
        let setup = std::iter::once(String::from(
            "data modify storage smelter:smelter internal_stack append value {}",
        ));
        let preamble = parameters
            .into_iter()
            .enumerate()
            .map(|(i, s)| format!("data modify storage smelter:smelter internal_stack[-1].{s} set from storage smelter:smelter fnargs[{i}]"));
        let teardown = std::iter::once(String::from(
            "data remove storage smelter:smelter internal_stack[-1]",
        ));
        Mcfunction {
            name: String::from(name),
            body: setup
                .chain(preamble)
                .chain(body.into_iter().map(|s| String::from(*s)))
                .chain(teardown)
                .collect(),
        }
    }
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
    let mut builder = DataPackBuilder::new(vec![
        // System calls
        Mcfunction::new_static(
            "syscall/initialize",
            &[],
            &[
                // Memory data structures
                "data modify storage smelter:smelter heap set value []",
                "data modify storage smelter:smelter job_queue set value []",
                "data modify storage smelter:smelter execution_stack set value []",
                "data modify storage smelter:smelter internal_stack set value []",
                // Registers
                "data modify storage smelter:smelter fnargs set value []",
                "data modify storage smelter:smelter fnret set value {}",
                "data modify storage smelter:smelter macroargs set value {}",
            ],
        ),
        Mcfunction::new_static(
            "syscall/allocate_on_heap",
            &["value"],
            &[
                "data modify storage smelter:smelter fnret set value {}",
                "execute store result storage smelter:smelter fnret.pointer int 1 run data get storage smelter:smelter heap",
                "data modify storage smelter:smelter heap append from storage smelter:smelter internal_stack[-1].value",
            ],
        ),
        Mcfunction::new_static(
            "syscall/macro_dereference_heap",
            &[],
            &[
                "data modify storage smelter:smelter $(output_path) set from storage smelter:smelter heap[$(heap_index)]",
            ],
        ),
        // Abstract operations
        Mcfunction::new_static(
            "absop/get_identifier_reference",
            &["env", "name"],
            &[
                // If env is null, return UNRESOLVABLE
                "execute if data storage smelter:smelter internal_stack[-1].env.null run data modify storage smelter:smelter fnargs set value [{record_type: {ReferenceRecord: true}, Base: {UNRESOLVABLE: true}, ThisValue: {EMPTY: true}}]",
                "execute if data storage smelter:smelter internal_stack[-1].env.null run data modify storage smelter:smelter fnargs[0].ReferencedName set from storage smelter:smelter internal_stack[-1].name",
                "execute if data storage smelter:smelter internal_stack[-1].env.null run function smelter:syscall/allocate_on_heap",
                "execute if data storage smelter:smelter internal_stack[-1].env.null run data modify storage smelter:smelter internal_stack[-1]._pointer set from storage smelter:smelter fnret.pointer",
                "execute if data storage smelter:smelter internal_stack[-1].env.null run data modify storage smelter:smelter fnret set value {Type: {NORMAL: true}, Value: {}}",
                "execute if data storage smelter:smelter internal_stack[-1].env.null run data modify storage smelter:smelter fnret.Value.record set from storage smelter:smelter internal_stack[-1]._pointer",
                "execute if data storage smelter:smelter internal_stack[-1].env.null run return 1",
                // Let exists be ?env.HasBinding(name)
                "data modify storage smelter:smelter fnargs set value []",
                "data modify storage smelter:smelter fnargs append from internal_stack[-1].env",
                "data modify storage smelter:smelter fnargs append from internal_stack[-1].name",
                "function smelter:absop/has_binding",
                "execute unless data storage smelter:smelter fnret.Type.NORMAL run return 1",
                "data modify storage smelter:smelter internal_stack[-1].exists set from storage smelter:smelter fnret.CompletionRecord.Value",
                // If exists is true, return resolved
                "execute if data storage smelter:smelter internal_stack[-1].exists.boolean.true run data modify storage smelter:smelter fnargs set value [{record_type: {ReferenceRecord: true}, ThisValue: {EMPTY: true}}]",
                "execute if data storage smelter:smelter internal_stack[-1].exists.boolean.true run data modify storage smelter:smelter fnargs[0].Base set from storage smelter:smelter internal_stack[-1].env",
                "execute if data storage smelter:smelter internal_stack[-1].exists.boolean.true run function smelter:syscall/allocate_on_heap",
                "execute if data storage smelter:smelter internal_stack[-1].exists.boolean.true run data modify storage smelter:smelter internal_stack[-1]._pointer set from storage smelter:smelter fnret.pointer",
                "execute if data storage smelter:smelter internal_stack[-1].exists.boolean.true run data modify storage smelter:smelter fnret set value {Type: {NORMAL: true}, Value: {}}",
                "execute if data storage smelter:smelter internal_stack[-1].exists.boolean.true run data modify storage smelter:smelter fnret.Value.record set from storage smelter:smelter internal_stack[-1]._pointer",
                "execute if data storage smelter:smelter internal_stack[-1].exists.boolean.true run return 1",
                // Else, recursion on env.[[OuterEnv]]
                "data modify storage smelter:smelter macroargs set value {output_path: 'internal_stack[-1]._deref_env'}",
                "data modify storage smelter:smelter macroargs.heap_index set from storage smelter:smelter internal_stack[-1].env.record",
                "function smelter:syscall/macro_dereference_heap with storage smelter:smelter macroargs",
                "data modify storage smelter:smelter internal_stack[-1].outer set from storage smelter:smelter internal_stack[-1]._deref_env.OuterEnv",
                "data modify storage smelter:smelter fnargs set value []",
                "data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].outer",
                "data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].name",
                "function smelter:absop/get_identifier_reference",
            ],
        ),
        Mcfunction::new_static(
            "absop/has_binding",
            &["env", "N"],
            &[
                "data modify storage smelter:smelter macroargs set value {output_path: 'internal_stack[-1]._deref_env'}",
                "data modify storage smelter:smelter macroargs.heap_index set from storage smelter:smelter internal_stack[-1].env.record",
                "function smelter:syscall/macro_dereference_heap_field with storage smelter:smelter macroargs",
                // DeclarativeEnvironmentRecord
                "execute if data storage smelter:smelter internal_stack[-1]._deref_env.record_type.DeclarativeEnvironmentRecord run data modify storage smelter:smelter macroargs set value {}",
                "execute if data storage smelter:smelter internal_stack[-1]._deref_env.record_type.DeclarativeEnvironmentRecord run data modify storage smelter:smelter macroargs.N set from storage smelter:smelter internal_stack[-1].N.string",
                "execute if data storage smelter:smelter internal_stack[-1]._deref_env.record_type.DeclarativeEnvironmentRecord run function smelter:has_binding__declarative",
                // FunctionEnvironmentRecord
                "execute if data storage smelter:smelter internal_stack[-1]._deref_env.record_type.FunctionEnvironmentRecord run data modify storage smelter:smelter macroargs set value {}",
                "execute if data storage smelter:smelter internal_stack[-1]._deref_env.record_type.FunctionEnvironmentRecord run data modify storage smelter:smelter macroargs.N set from storage smelter:smelter internal_stack[-1].N.string",
                "execute if data storage smelter:smelter internal_stack[-1]._deref_env.record_type.FunctionEnvironmentRecord run function smelter:has_binding__declarative",
                // Return completion
                "data modify storage smelter:smelter internal_stack[-1]._value set from storage smelter:smelter fnret",
                "data modify storage smelter:smelter fnret set value {Type: {NORMAL: true}, Value: {}}",
                "data modify storage smelter:smelter fnret.Value set from storage smelter:smelter internal_stack[-1]._value",
            ],
        ),
        Mcfunction::new_static(
            "absop/has_binding__declarative",
            &[],
            &[
                "data modify storage smelter:smelter fnret set value {boolean: {false: true}}",
                "execute if data storage smelter:smelter internal_stack[-1]._deref_env.bindings.$(N) run data modify storage smelter:smelter fnret set value {boolean: {true: true}}",
            ],
        ),
        Mcfunction::new_static(
            "absop/resolve_binding",
            &["name", "env"],
            &[
                // If env not provided, set to running execution context's LexicalEnvironment
                "execute unless data storage smelter:smelter internal_stack[-1].env.record run data modify storage smelter:smelter internal_stack[-1].env set from storage smelter:smelter execution_stack[-1].LexicalEnvironment",
                // Call GetIdentifierReference and return result directly
                "data modify storage smelter:smelter fnargs set value []",
                "data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].env",
                "data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].name",
                "function smelter:absop/get_identifier_reference",
            ],
        ),
    ]);

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
        Statement::ExpressionStatement(expression_statement) => {
            let expression_id = make_expression_id(&expression_statement.span);
            compile_expression(builder, &expression_statement.expression, &expression_id)?
        }
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
        Statement::ClassDeclaration(class) => todo!(),
        Statement::FunctionDeclaration(function) => todo!(),
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

fn make_expression_id(span: &Span) -> String {
    format!("expr_{}", span.start)
}

fn compile_expression(
    builder: &mut DataPackBuilder,
    expression: &Expression,
    expression_id: &String,
) -> Result<()> {
    match expression {
        // Literal values
        Expression::ArrayExpression(array_expression) => todo!(),
        Expression::BigIntLiteral(big_int_literal) => todo!(),
        Expression::BooleanLiteral(boolean_literal) => todo!(),
        Expression::NullLiteral(null_literal) => todo!(),
        Expression::NumericLiteral(numeric_literal) => todo!(),
        Expression::ObjectExpression(object_expression) => todo!(),
        Expression::RegExpLiteral(reg_exp_literal) => todo!(),
        Expression::StringLiteral(string_literal) => todo!(),
        Expression::TemplateLiteral(template_literal) => todo!(),
        // Functions and classes
        Expression::ArrowFunctionExpression(arrow_function_expression) => todo!(),
        Expression::ClassExpression(class) => todo!(),
        Expression::FunctionExpression(function) => todo!(),
        // References
        Expression::Identifier(identifier_reference) => {
            let string_value = identifier_reference.name.as_str();
            builder.extend_commands(vec![
                debug_log(format!("evaluating identifier {string_value}")),
                format!("data modify storage smelter:smelter fnargs set value [{{string: '{string_value}'}}]"),
                String::from("function smelter:absop/resolve_binding"),
                String::from("execute unless data storage smelter:smelter fnret.Type.NORMAL run return 1"),
                format!("data modify storage smelter:smelter execution_stack[-1].evaluations.{expression_id} set from storage smelter:smelter fnret.Value"),
                debug_log(format!("done evaluating identifier {string_value}")),
            ]);
        }
        Expression::MetaProperty(meta_property) => todo!(),
        Expression::Super(sooper) => todo!(),
        Expression::ThisExpression(this_expression) => todo!(),
        // Assignments
        Expression::AssignmentExpression(assignment_expression) => todo!(),
        // Member access
        Expression::ChainExpression(chain_expression) => todo!(),
        Expression::ComputedMemberExpression(computed_member_expression) => todo!(),
        Expression::PrivateFieldExpression(private_field_expression) => todo!(),
        Expression::StaticMemberExpression(static_member_expression) => todo!(),
        // Operations
        Expression::BinaryExpression(binary_expression) => todo!(),
        Expression::LogicalExpression(logical_expression) => todo!(),
        Expression::PrivateInExpression(private_in_expression) => todo!(),
        Expression::UnaryExpression(unary_expression) => todo!(),
        Expression::UpdateExpression(update_expression) => todo!(),
        // Function calls
        Expression::CallExpression(call_expression) => todo!(),
        Expression::NewExpression(new_expression) => todo!(),
        Expression::TaggedTemplateExpression(tagged_template_expression) => todo!(),
        // Control flow
        Expression::AwaitExpression(await_expression) => todo!(),
        Expression::ConditionalExpression(conditional_expression) => todo!(),
        Expression::YieldExpression(yield_expression) => todo!(),
        // Organization
        Expression::ImportExpression(_) => bail!("Not supported: import expressions"),
        Expression::ParenthesizedExpression(parenthesized_expression) => todo!(),
        Expression::SequenceExpression(sequence_expression) => todo!(),
        // JSX, TypeScript, and other language extensions
        Expression::JSXElement(_) => bail!("Not supported: JSX expressions"),
        Expression::JSXFragment(_) => bail!("Not supported: JSX expressions"),
        Expression::TSAsExpression(_) => (),
        Expression::TSInstantiationExpression(_) => (),
        Expression::TSNonNullExpression(_) => (),
        Expression::TSSatisfiesExpression(_) => (),
        Expression::TSTypeAssertion(_) => (),
        Expression::V8IntrinsicExpression(_) => bail!("Not supported: V8 intrinsics"),
    }
    Ok(())
}
