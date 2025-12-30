use anyhow::{Context, Result, anyhow, bail};
use clap::Parser as CliParser;
use oxc::{
    allocator::{Allocator, Box as OxcBox},
    ast::ast::{
        BindingIdentifier, BindingPattern, BindingPatternKind, BindingRestElement, Expression,
        FormalParameters, FunctionBody, Program, Statement,
    },
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
        Mcfunction {
            name: String::from("initialize"),
            body: vec![
                // Memory data structures
                String::from(
                    "data modify storage smelter:smelter heap set value [{parent: -1, bindings: {}, evaluations: {}}]",
                ),
                String::from("data modify storage smelter:smelter queue set value []"),
                String::from("data modify storage smelter:smelter stack set value []"),
                // Registers
                String::from("data modify storage smelter:smelter fnenvptr set value 0"),
                String::from("data modify storage smelter:smelter fnargs set value []"),
                String::from("data modify storage smelter:smelter fnret set value {}"),
                String::from("data modify storage smelter:smelter sysargs set value {}"),
                String::from("data modify storage smelter:smelter sysret set value {}"),
                // Arithmetic
                String::from("scoreboard objectives add smelter_internal dummy"),
            ],
        },
        Mcfunction {
            name: String::from("resolve_identifier"),
            body: vec![
                debug_log_macro(String::from(
                    "entering resolve_identifier: identifier=$(identifier), heap_index=$(heap_index)",
                )),
                // Clear sysret register
                String::from("data modify storage smelter:smelter sysret set value {}"),
                // If binding exists in this environment, then return resolved
                String::from(
                    "$execute if data storage smelter:smelter heap[$(heap_index)].bindings.$(identifier) run data modify storage smelter:smelter sysret set value {resolved: {env: $(heap_index)}}",
                ),
                String::from(
                    "$execute if data storage smelter:smelter heap[$(heap_index)].bindings.$(identifier) run data modify storage smelter:smelter sysret.resolved.value set from storage smelter:smelter heap[$(heap_index)].bindings.$(identifier)",
                ),
                debug_log(String::from(
                    "resolve_identifier: checking if binding exists",
                )),
                String::from(
                    "$execute if data storage smelter:smelter heap[$(heap_index)].bindings.$(identifier) run return 1",
                ),
                // Else if no parent, return fail
                String::from(
                    "$execute store result score #resolve_identifier__parent_index smelter_internal run data get storage smelter:smelter heap[$(heap_index)].parent",
                ),
                debug_log(String::from(
                    "resolve_identifier: checking if parent exists",
                )),
                String::from(
                    "execute if score #resolve_identifier__parent_index smelter_internal matches ..-1 run return fail",
                ),
                // Else, repeat on parent
                String::from(
                    "execute store result storage smelter:smelter sysargs.heap_index int 1 run scoreboard players get #resolve_identifier__parent_index smelter_internal",
                ),
                debug_log(String::from("resolve_identifier: calling on parent")),
                String::from(
                    "return run function smelter:resolve_identifier with storage smelter:smelter sysargs",
                ),
            ],
        },
        Mcfunction {
            name: String::from("write_binding"),
            body: vec![
                debug_log_macro(String::from(
                    "entering write_binding: heap_index=$(heap_index), identifier=$(identifier), value=$(value)",
                )),
                String::from(
                    "$data modify storage smelter:smelter heap[$(heap_index)].bindings.$(identifier) set value $(value)",
                ),
                debug_log(String::from("exiting write_binding")),
            ],
        },
        Mcfunction {
            name: String::from("write_evaluation"),
            body: vec![
                debug_log_macro(String::from(
                    "entering write_evaluation: heap_index=$(heap_index), expression_id=$(expression_id), value=$(value)",
                )),
                String::from(
                    "$data modify storage smelter:smelter heap[$(heap_index)].evaluations.$(expression_id) set value $(value)",
                ),
                debug_log(String::from("exiting write_evaluation")),
            ],
        },
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
            compile_expression(builder, &expression_statement.expression, expression_id)?
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
        Statement::ClassDeclaration(class_declaration) => todo!(),
        Statement::FunctionDeclaration(function) => {
            // TypeScript function overloads don't have a body
            if let Some(body) = &function.body {
                let file_name = make_function_file_name(&function.id, &function.span);
                let function_name = function
                    .id
                    .as_ref()
                    .ok_or(anyhow!("Function declaration with no identifier"))?
                    .name
                    .to_string();
                compile_function(builder, file_name.clone(), &function.params, body)?;
                builder.extend_commands(vec![
                    debug_log(format!("evaluating function declaration {file_name}")),
                    format!("data modify storage smelter:smelter sysargs set value {{identifier: '{function_name}', value: {{function: {{file_name: '{file_name}', function_name: '{function_name}'}}}}}}"),
                    String::from("data modify storage smelter:smelter sysargs.value.function.parent_env set from storage smelter:smelter fnenvptr"),
                    String::from("data modify storage smelter:smelter sysargs.heap_index set from storage smelter:smelter fnenvptr"),
                    String::from("function smelter:write_binding with storage smelter:smelter sysargs"),
                    debug_log(format!("done evaluating function declaration {file_name}")),
                ]);
            }
        }
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
    expression_id: String,
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
        Expression::StringLiteral(string_literal) => {
            let string_value = string_literal.value.as_str();
            builder.extend_commands(vec![
                debug_log(format!("evaluating string literal {string_value}")),
                format!("data modify storage smelter:smelter sysargs set value {{expression_id: '{expression_id}', value: {{string: '{string_value}'}}}}"),
                String::from("data modify storage smelter:smelter sysargs.heap_index set from storage smelter:smelter fnenvptr"),
                String::from("function smelter:write_evaluation with storage smelter:smelter sysargs"),
                debug_log(format!("done evaluating string literal {string_value}")),
            ]);
        }
        Expression::TemplateLiteral(template_literal) => todo!(),
        // Functions and classes
        Expression::ArrowFunctionExpression(arrow_function_expression) => {
            let file_name = make_function_file_name(&None, &arrow_function_expression.span);
            compile_function(
                builder,
                file_name.clone(),
                &arrow_function_expression.params,
                &arrow_function_expression.body,
            )?;
            builder.extend_commands(vec![
                debug_log(format!("evaluating arrow function expression {file_name}")),
                format!("data modify storage smelter:smelter sysargs set value {{expression_id: '{expression_id}', value: {{function: {{file_name: '{file_name}'}}}}}}"),
                String::from("data modify storage smelter:smelter sysargs.value.function.parent_env set from storage smelter:smelter fnenvptr"),
                String::from("data modify storage smelter:smelter sysargs.heap_index set from storage smelter:smelter fnenvptr"),
                String::from("function smelter:write_evaluation with storage smelter:smelter sysargs"),
                debug_log(format!("done evaluating arrow function expression {file_name}")),
            ]);
        }
        Expression::ClassExpression(class_expression) => todo!(),
        Expression::FunctionExpression(function) => {
            if let Some(body) = &function.body {
                let file_name = make_function_file_name(&function.id, &function.span);
                let function_name = &function.id.as_ref().map(|id| id.name.to_string());
                compile_function(builder, file_name.clone(), &function.params, body)?;
                builder.extend_commands(vec![
                    debug_log(format!("evaluating function expression {file_name}")),
                    format!("data modify storage smelter:smelter sysargs set value {{expression_id: '{expression_id}', value: {{function: {{file_name: '{file_name}'}}}}}}"),
                ]);
                if let Some(name) = function_name {
                    builder.push_command(format!("data modify storage smelter:smelter sysargs.value.function.function_name set value '{name}'"));
                }
                builder.extend_commands(vec![
                    String::from("data modify storage smelter:smelter sysargs.value.function.parent_env set from storage smelter:smelter fnenvptr"),
                    String::from("data modify storage smelter:smelter sysargs.heap_index set from storage smelter:smelter fnenvptr"),
                    String::from("function smelter:write_evaluation with storage smelter:smelter sysargs"),
                    debug_log(format!("done evaluating function expression {file_name}")),
                ]);
            }
        }
        // References
        Expression::Identifier(identifier_reference) => {
            let identifier = identifier_reference.name.as_str();
            builder.extend_commands(vec![
                debug_log(format!("evaluating identifier {identifier}")),
                // Resolve identifier
                format!("data modify storage smelter:smelter sysargs set value {{identifier: '{identifier}'}}"),
                format!("data modify storage smelter:smelter sysargs.heap_index set from storage smelter:smelter fnenvptr"),
                String::from("function smelter:resolve_identifier with storage smelter:smelter sysargs"),
                // Throw if unresolved
                format!("execute unless data storage smelter:smelter sysret.resolved run data modify storage smelter:smelter fnret set value {{throw: {{type: 'ReferenceError', message: 'No binding exists for identifier {identifier}'}}}}"),
                String::from("execute unless data storage smelter:smelter sysret.resolved run return fail"),
                // Write evaluation
                format!("data modify storage smelter:smelter sysargs set value {{expression_id: '{expression_id}'}}"),
                String::from("data modify storage smelter:smelter sysargs.heap_index set from storage smelter:smelter fnenvptr"),
                String::from("data modify storage smelter:smelter sysargs.value set from storage smelter:smelter sysret.resolved.value"),
                String::from("function smelter:write_evaluation with storage smelter:smelter sysargs"),
                debug_log(format!("done evaluating identifier {identifier}")),
            ]);
        }
        Expression::MetaProperty(meta_property) => todo!(),
        Expression::Super(sooper) => todo!(),
        Expression::ThisExpression(this_expression) => todo!(),
        // Assignments
        Expression::AssignmentExpression(assignment_expression) => todo!(),
        // Member access
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
        Expression::ChainExpression(chain_expression) => todo!(),
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

fn make_function_file_name(id: &Option<BindingIdentifier>, span: &Span) -> String {
    if let Some(identifier) = id {
        format!("{}_{}", identifier.name.to_ascii_lowercase(), span.start)
    } else {
        format!("anon_func_{}", span.start)
    }
}

fn compile_function(
    builder: &mut DataPackBuilder,
    file_name: String,
    parameters: &OxcBox<FormalParameters>,
    body: &OxcBox<FunctionBody>,
) -> Result<()> {
    // Raw command interface
    let command_directive = body
        .directives
        .iter()
        .find(|directive| directive.directive.starts_with("smelter command"));
    if let Some(directive) = command_directive {
        let tokens = directive.directive.split(' ').collect::<Vec<&str>>();
        if let Some(command) = tokens.get(2) {
            builder
                .push_mcfunction(file_name)
                .extend_commands(vec![
                    debug_log(format!("entering wrapper function {command}")),
                    String::from("execute unless data storage smelter:smelter fnargs[0].string run return fail"),
                    String::from("data modify storage smelter:smelter sysargs.tail set from storage smelter:smelter fnargs[0].string"),
                    debug_log(format!("returning from wrapper function {command}")),
                    format!("return run function smelter:{command}_macro with storage smelter:smelter sysargs"),
                ])
                .commit_mcfunction();
            builder
                .push_mcfunction(format!("{command}_macro"))
                .extend_commands(vec![
                    debug_log_macro(format!("entering macro function {command}: tail=$(tail)")),
                    format!("${command} $(tail)"),
                    debug_log(format!("exiting macro function {command}")),
                ])
                .commit_mcfunction();
            return Ok(());
        }
    }

    // Normal JS function
    builder
        .push_mcfunction(file_name.clone())
        .push_command(format!("entering function {file_name}"));

    // Bind arguments
    for parameter in parameters.items.iter() {
        compile_bind_argument(builder, &parameter.pattern)?;
    }
    if let Some(rest_parameter) = &parameters.rest {
        compile_bind_rest_argument(builder, &rest_parameter)?;
    }

    // Compile body
    for statement in &body.statements {
        compile_statement(builder, statement)?;
    }

    builder
        .push_command(format!("exiting function {file_name}"))
        .commit_mcfunction();
    Ok(())
}

fn compile_bind_argument(builder: &mut DataPackBuilder, pattern: &BindingPattern) -> Result<()> {
    match &pattern.kind {
        BindingPatternKind::ArrayPattern(array_pattern) => todo!(),
        BindingPatternKind::AssignmentPattern(assignment_pattern) => todo!(),
        BindingPatternKind::BindingIdentifier(bi) => {
            let name = bi.name.as_str();
            builder.extend_commands(vec![
                debug_log(format!("binding argument {name}")),
                format!("data modify storage smelter:smelter sysargs set value {{identifier: '{name}'}}"),
                String::from("data modify storage smelter:smelter sysargs.heap_index set from storage smelter:smelter fnenvptr"),
                format!("execute unless data storage smelter:smelter fnargs[0] run data modify storage smelter:smelter sysargs.value set value {{undefined: true}}"),
                format!("execute if data storage smelter:smelter fnargs[0] run data modify storage smelter:smelter sysargs.value set from storage smelter:smelter fnargs[0]"),
                String::from("function smelter:write_binding with storage smelter:smelter sysargs"),
                String::from("data remove storage smelter:smelter fnargs[0]"),
                debug_log(format!("done binding argument {name}")),
            ]);
        }
        BindingPatternKind::ObjectPattern(object_pattern) => todo!(),
    }
    Ok(())
}

fn compile_bind_rest_argument(
    builder: &mut DataPackBuilder,
    pattern: &BindingRestElement,
) -> Result<()> {
    let name = pattern
        .argument
        .get_identifier_name()
        .map_or(String::from(""), |atom| atom.into_string());
    builder.extend_commands(vec![
        debug_log(format!("binding rest argument {name}")),
        format!("data modify storage smelter:smelter sysargs set value {{identifier: '{name}'}}"),
        String::from("data modify storage smelter:smelter sysargs.heap_index set from storage smelter:smelter fnenvptr"),
        String::from("data modify storage smelter:smelter sysargs.value set from storage smelter:smelter fnargs"),
        String::from("function smelter:write_binding with storage smelter:smelter sysargs"),
        String::from("data modify storage smelter:smelter fnargs set value []"),
        debug_log(format!("done binding rest argument {name}")),
    ]);
    Ok(())
}
