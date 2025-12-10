use anyhow::{Context, Result};
use clap::Parser as CliParser;
use oxc::{
    allocator::{Allocator, Box as OxcBox},
    ast::ast::{
        ArrowFunctionExpression, BindingIdentifier, BindingPattern, BindingPatternKind,
        BindingRestElement, CallExpression, Expression, FormalParameters, Function, FunctionBody,
        Program, Statement,
    },
    ast_visit::Visit,
    parser::Parser,
    semantic::{ScopeFlags, SemanticBuilder},
    span::{GetSpan, SourceType, Span},
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

    let data_pack = compile_program(program);
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

struct FunctionCompiler {
    functions: Vec<Mcfunction>,
}

impl Visit<'_> for FunctionCompiler {
    fn visit_function(&mut self, it: &Function<'_>, _: ScopeFlags) {
        if let Some(body) = &it.body {
            self.functions
                .extend(compile_function(&it.id, &it.params, body, &it.span));
        }
    }

    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'_>) {
        self.functions
            .extend(compile_function(&None, &it.params, &it.body, &it.span))
    }
}

fn debug_log(message: String) -> String {
    format!(
        "execute if score #debug smelter_internal matches 1.. run tellraw @a '[smelter] {message}'"
    )
}

fn compile_program(program: Program) -> DataPack {
    let core_functions: Vec<Mcfunction> = vec![
        compile_init_function(),
        compile_identifier_resolution(),
        compile_function_invocation(),
        compile_stack_pop(),
    ];
    let mut function_compiler = FunctionCompiler {
        functions: Vec::new(),
    };
    oxc::ast_visit::walk::walk_program(&mut function_compiler, &program);
    let (mut main_function_body, subfunctions) = reduce_compiled(
        program
            .body
            .iter()
            .map(|statement| compile_statement(statement))
            .collect::<Vec<(Vec<String>, Vec<Mcfunction>)>>(),
    );
    main_function_body.insert(0, debug_log(String::from("entering main")));
    main_function_body.push(debug_log(String::from("exiting main")));
    function_compiler
        .functions
        .into_iter()
        .chain(core_functions.into_iter())
        .chain(subfunctions.into_iter())
        .chain(std::iter::once(Mcfunction {
            name: String::from("main"),
            body: main_function_body,
        }))
        .collect()
}

fn make_function_name(id: &Option<BindingIdentifier>, span: &Span) -> String {
    if let Some(identifier) = id {
        format!("{}_{}", identifier.name.to_ascii_lowercase(), span.start)
    } else {
        format!("anon_func_{}", span.start)
    }
}

fn compile_function(
    id: &Option<BindingIdentifier>,
    parameters: &OxcBox<FormalParameters>,
    body: &OxcBox<FunctionBody>,
    span: &Span,
) -> Vec<Mcfunction> {
    let function_name = make_function_name(id, span);

    let command_directive = body
        .directives
        .iter()
        .find(|directive| directive.directive.starts_with("smelter"));
    if let Some(directive) = command_directive {
        let tokens = directive.directive.split(' ').collect::<Vec<&str>>();
        if let Some(command) = tokens.get(1) {
            return vec![
                Mcfunction {
                    name: function_name,
                    body: compile_command_wrapper_function_body(command),
                },
                compile_command_macro_function(command),
            ];
        }
    }

    let mut compiled_body: Vec<String> = Vec::new();
    let mut subfunctions: Vec<Mcfunction> = Vec::new();
    compiled_body.push(debug_log(format!("entering function {function_name}")));

    // Copy arguments into bindings
    for parameter in parameters.items.iter() {
        compiled_body.extend(compile_bind_argument(&parameter.pattern));
    }
    if let Some(rest_parameter) = &parameters.rest {
        compiled_body.extend(compile_bind_rest_argument(&rest_parameter));
    }

    // Evaluate body statements
    for statement in &body.statements {
        let result = compile_statement(statement);
        compiled_body.extend(result.0);
        subfunctions.extend(result.1);
    }

    compiled_body.push(debug_log(format!("exiting function {function_name}")));
    subfunctions
        .into_iter()
        .chain(std::iter::once(Mcfunction {
            name: function_name,
            body: compiled_body,
        }))
        .collect()
}

fn compile_command_wrapper_function_body(command: &str) -> Vec<String> {
    vec![
        debug_log(format!("entering wrapper function {command}")),
        String::from(
            "execute unless data storage smelter:smelter current_arguments[0].string run data modify storage smelter:smelter current_return_value set value {throw: 'TypeError'}",
        ),
        String::from(
            "execute unless data storage smelter:smelter current_arguments[0].string run return fail",
        ),
        String::from(
            "data modify storage smelter:smelter internal.command_args.tail set from storage smelter:smelter current_arguments[0].string",
        ),
        debug_log(format!("returning from wrapper function {command}")),
        format!(
            "return run function smelter:{command}_macro with storage smelter:smelter internal.command_args"
        ),
    ]
}

fn compile_command_macro_function(command: &str) -> Mcfunction {
    Mcfunction {
        name: format!("{command}_macro"),
        body: vec![
            format!(
                "${}",
                debug_log(format!("entering macro function {command}: tail=$(tail)"))
            ),
            format!("${command} $(tail)"),
            debug_log(format!("exiting macro function {command}")),
        ],
    }
}

fn compile_init_function() -> Mcfunction {
    Mcfunction {
        name: String::from("initialize"),
        body: vec![
            String::from("data modify storage smelter:smelter environment_stack set value []"),
            String::from("data modify storage smelter:smelter current_arguments set value []"),
            String::from(
                "data modify storage smelter:smelter current_environment set value {parent: -1, bindings: {}, evaluations: {}}",
            ),
            String::from("data modify storage smelter:smelter current_return_value set value {}"),
            String::from("data modify storage smelter:smelter internal set value {}"),
            String::from("scoreboard objectives add smelter_internal dummy"),
        ],
    }
}

fn compile_bind_argument(pattern: &BindingPattern) -> Vec<String> {
    match &pattern.kind {
        BindingPatternKind::BindingIdentifier(bi) => {
            let name = bi.name.as_str();
            vec![
                debug_log(format!("binding argument {name}")),
                format!(
                    "execute unless data storage smelter:smelter current_arguments[0] run data modify storage smelter:smelter current_environment.bindings.{name} set value {{undefined: true}}",
                ),
                format!(
                    "execute if data storage smelter:smelter current_arguments[0] run data modify storage smelter:smelter current_environment.bindings.{name} set from storage smelter:smelter current_arguments[0]",
                ),
                String::from("data remove storage smelter:smelter current_arguments[0]"),
                debug_log(format!("done binding argument {name}")),
            ]
        }
        _ => Vec::new(),
    }
}

fn compile_bind_rest_argument(pattern: &BindingRestElement) -> Vec<String> {
    let name = pattern
        .argument
        .get_identifier_name()
        .map_or(String::from(""), |atom| atom.into_string());
    vec![
        debug_log(format!("binding rest argument {name}")),
        format!(
            "data modify storage smelter:smelter current_environment.bindings.{name} set from storage smelter:smelter current_arguments",
        ),
        debug_log(format!("done binding rest argument {name}")),
    ]
}

fn compile_statement(statement: &Statement) -> (Vec<String>, Vec<Mcfunction>) {
    match statement {
        Statement::ExpressionStatement(expr_stmt) => compile_expression(&expr_stmt.expression),
        Statement::FunctionDeclaration(function) => {
            if let Some(_) = &function.body {
                let function_name = make_function_name(&function.id, &function.span);
                let function_identifier = function.id.as_ref().unwrap().name.to_string();
                (
                    vec![
                        debug_log(format!("evaluating function declaration {function_name}")),
                        format!(
                            "data modify storage smelter:smelter current_environment.bindings.{function_identifier} set value {{function: {{name: '{function_name}'}}}}"
                        ),
                        format!(
                            "execute store result storage smelter:smelter current_environment.bindings.{function_identifier}.function.environment_index int 1 run data get storage smelter:smelter environment_stack"
                        ),
                        debug_log(format!(
                            "done evaluating function declaration {function_name}"
                        )),
                    ],
                    Vec::new(),
                )
            } else {
                (Vec::new(), Vec::new())
            }
        }
        Statement::VariableDeclaration(declaration) => {
            let mut commands: Vec<String> = Vec::new();
            let mut subfunctions: Vec<Mcfunction> = Vec::new();
            for declarator in &declaration.declarations {
                if let Some(identifier) = declarator.id.get_identifier_name() {
                    let name = identifier.as_str();
                    commands.push(debug_log(format!("evaluating variable declaration {name}")));
                    if let Some(initializer) = &declarator.init {
                        // Compile initializer
                        let expression_id = make_expression_id(initializer);
                        let compiled = compile_expression(initializer);
                        commands.extend(compiled.0);
                        subfunctions.extend(compiled.1);
                        commands.push(format!("data modify storage smelter:smelter current_environment.bindings.{name} set from storage smelter:smelter current_environment.evaluations.{expression_id}"));
                    } else {
                        // Initialize to undefined
                        commands.push(format!("data modify storage smelter:smelter current_environment.bindings.{name} set value {{undefined: true}}"));
                    }
                    commands.push(debug_log(format!(
                        "done evaluating variable declaration {name}"
                    )));
                }
            }
            (commands, subfunctions)
        }
        _ => (Vec::new(), Vec::new()),
    }
}

fn make_expression_id(expression: &Expression) -> String {
    format!("expr_{}", expression.span().start)
}

fn compile_expression(expression: &Expression) -> (Vec<String>, Vec<Mcfunction>) {
    let expression_id = make_expression_id(expression);
    match expression {
        Expression::ArrowFunctionExpression(arrow_func) => {
            let function_name = make_function_name(&None, &arrow_func.span);
            (
                vec![
                    debug_log(format!("evaluating arrow function {function_name}")),
                    // Set function object
                    format!(
                        "data modify storage smelter:smelter current_environment.evaluations.{expression_id} set value {{function: {{name: '{function_name}'}}}}"
                    ),
                    // Store pointer to end of environment stack (where current environment will be pushed if this function gets called by the current function)
                    format!(
                        "execute store result storage smelter:smelter current_environment.evaluations.{expression_id}.function.environment_index int 1 run data get storage smelter:smelter environment_stack"
                    ),
                    debug_log(format!("done evaluating arrow function {function_name}")),
                ],
                Vec::new(),
            )
        }
        Expression::Identifier(ident_ref) => {
            let identifier = ident_ref.name.to_string();
            (
                vec![
                    debug_log(format!("evaluating identifier {identifier}")),
                    // If binding exists in current environment, then copy value to evaluation
                    format!(
                        "execute if data storage smelter:smelter current_environment.bindings.{identifier} run data modify storage smelter:smelter current_environment.evaluations.{expression_id} set from storage smelter:smelter current_environment.bindings.{identifier}"
                    ),
                    // Else, run `resolve`
                    format!(
                        "execute unless data storage smelter:smelter current_environment.evaluations.{expression_id} run data modify storage smelter:smelter internal.resolve_args set value {{identifier: '{identifier}', expression_id: '{expression_id}'}}"
                    ),
                    format!(
                        "execute unless data storage smelter:smelter current_environment.evaluations.{expression_id} run data modify storage smelter:smelter internal.resolve_args.stack_index set from storage smelter:smelter current_environment.parent"
                    ),
                    format!(
                        "execute unless data storage smelter:smelter current_environment.evaluations.{expression_id} run function smelter:resolve with storage smelter:smelter internal.resolve_args"
                    ),
                    debug_log(format!("done evaluating identifier {identifier}")),
                ],
                vec![],
            )
        }
        Expression::StringLiteral(literal) => {
            let string_value = literal.value.as_str();
            (
                vec![
                    debug_log(format!("evaluating string literal {string_value}")),
                    format!(
                        "data modify storage smelter:smelter current_environment.evaluations.{expression_id} set value {{string: '{string_value}'}}"
                    ),
                    debug_log(format!("done evaluating string literal {string_value}")),
                ],
                Vec::new(),
            )
        }
        Expression::CallExpression(call_expr) => compile_call_expression(&call_expr),
        _ => (Vec::new(), Vec::new()),
    }
}

fn compile_identifier_resolution() -> Mcfunction {
    Mcfunction {
        name: String::from("resolve"),
        body: vec![
            format!(
                "${}",
                debug_log(format!(
                    "entering resolve: identifier=$(identifier) stack_index=$(stack_index) expression_id=$(expression_id)"
                ))
            ),
            // If binding exists at this index in environment stack, then copy value to target location and return
            String::from(
                "$execute if data storage smelter:smelter environment_stack[$(stack_index)].bindings.$(identifier) run data modify storage smelter:smelter current_environment.evaluations.$(expression_id) set from storage smelter:smelter environment_stack[$(stack_index)].bindings.$(identifier)",
            ),
            debug_log(String::from("resolve: checking if binding exists here")),
            String::from(
                "$execute if data storage smelter:smelter environment_stack[$(stack_index)].bindings.$(identifier) run return 1",
            ),
            // Else if no parent, return fail
            String::from(
                "$execute store result score #resolve__parent_index smelter_internal run data get storage smelter:smelter environment_stack[$(stack_index)].parent",
            ),
            debug_log(String::from("resolve: checking if parent exists")),
            String::from(
                "execute if score #resolve__parent_index smelter_internal matches ..-1 run return fail",
            ),
            // Else, recurse on parent
            String::from(
                "execute store result storage smelter:smelter internal.resolve_args.stack_index int 1 run scoreboard players get #resolve__parent_index smelter_internal",
            ),
            debug_log(String::from("resolve: calling with parent")),
            String::from(
                "return run function smelter_resolve with storage smelter:smelter internal.resolve_args",
            ),
        ],
    }
}

fn compile_call_expression(expression: &CallExpression) -> (Vec<String>, Vec<Mcfunction>) {
    let callee_expr_id = make_expression_id(&expression.callee);
    // Evaluate callee first
    let mut compiled = vec![compile_expression(&expression.callee)];
    // Evaluate each argument
    compiled.extend(expression.arguments.iter().filter_map(|argument| {
        argument
            .as_expression()
            .map(|arg_expr| compile_expression(arg_expr))
    }));
    compiled.extend(expression.arguments.iter().map(|argument| (
        vec![
            // Copy arguments into register
            format!("data modify storage smelter:smelter current_arguments append from storage smelter:smelter current_environment.evaluations.expr_{}", argument.span().start)
        ],
        Vec::new(),
    )));
    compiled.push((vec![
        debug_log(format!("invoking function {callee_expr_id}")),
        // Push current environment onto stack
        String::from("data modify storage smelter:smelter environment_stack append from storage smelter:smelter current_environment"),
        // Invoke callee function
        format!("function smelter:invoke with storage smelter:smelter current_environment.evaluations.{callee_expr_id}.function"),
        // Pop environment
        String::from("data modify storage smelter:smelter internal.pop_stack_args set value {stack_size: 0}"),
        String::from("execute store result storage smelter:smelter internal.pop_stack_args.stack_size int 1 run data get storage smelter:smelter environment_stack"),
        String::from("function smelter:pop_stack with storage smelter:smelter internal.pop_stack_args"),
        // Clear arguments
        String::from("data modify storage smelter:smelter current_arguments set value []"),
        debug_log(format!("done invoking function {callee_expr_id}")),
    ], Vec::new()));
    reduce_compiled(compiled)
}

fn compile_function_invocation() -> Mcfunction {
    Mcfunction {
        name: String::from("invoke"),
        body: vec![
            format!(
                "${}",
                debug_log(String::from(
                    "entering invoke: name=$(name) environment_index=$(environment_index)"
                ))
            ),
            String::from(
                "$data modify storage smelter:smelter current_environment set from storage smelter:smelter environment_stack[$(environment_index)]",
            ),
            String::from("$function smelter:$(name)"),
            debug_log(String::from("exiting invoke")),
        ],
    }
}

fn compile_stack_pop() -> Mcfunction {
    Mcfunction {
        name: String::from("pop_stack"),
        body: vec![
            format!(
                "${}",
                debug_log(String::from("entering pop_stack: stack_size=$(stack_size)"))
            ),
            String::from(
                "$data modify storage smelter:smelter current_environment set from storage smelter:smelter environment_stack[$(stack_size)]",
            ),
            String::from("$data remove storage smelter:smelter environment_stack[$(stack_size)]"),
            debug_log(String::from("exiting pop_stack")),
        ],
    }
}

fn reduce_compiled(v: Vec<(Vec<String>, Vec<Mcfunction>)>) -> (Vec<String>, Vec<Mcfunction>) {
    v.into_iter().fold(
        (Vec::new(), Vec::new()),
        |(mut commands_acc, mut subfunctions_acc), (commands, subfunctions)| {
            commands_acc.extend(commands);
            subfunctions_acc.extend(subfunctions);
            (commands_acc, subfunctions_acc)
        },
    )
}
