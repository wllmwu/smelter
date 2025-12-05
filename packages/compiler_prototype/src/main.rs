use anyhow::{Context, Result};
use clap::Parser as CliParser;
use oxc::{
    allocator::{Allocator, Box as OxcBox},
    ast::ast::{
        ArrowFunctionExpression, BindingIdentifier, BindingPattern, BindingPatternKind,
        BindingRestElement, FormalParameters, Function, FunctionBody, Program,
    },
    ast_visit::Visit,
    parser::Parser,
    semantic::{ScopeFlags, SemanticBuilder},
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

    let data_pack = compile_program(program);
    for function in data_pack {
        std::fs::write(&function.name, function.body.join("\n"))
            .with_context(|| format!("Couldn't write file `{}`", &function.name))?
    }

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
    fn visit_function(&mut self, it: &Function<'_>, flags: ScopeFlags) {
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

fn compile_program(program: Program) -> DataPack {
    let mut function_compiler = FunctionCompiler {
        functions: Vec::new(),
    };
    oxc::ast_visit::walk::walk_program(&mut function_compiler, &program);
    function_compiler
        .functions
        .into_iter()
        .chain(std::iter::once(compile_init_function()))
        .collect()
}

fn compile_function(
    id: &Option<BindingIdentifier>,
    parameters: &OxcBox<FormalParameters>,
    body: &OxcBox<FunctionBody>,
    span: &Span,
) -> Vec<Mcfunction> {
    let function_name = if let Some(identifier) = id {
        format!("{}_{}.mcfunction", identifier.name.as_str(), span.start)
    } else {
        format!("anon_func_{}.mcfunction", span.start)
    };
    let mut compiled_body: Vec<String> = Vec::new();
    let mut subfunctions: Vec<Mcfunction> = Vec::new();

    for parameter in parameters.items.iter() {
        compiled_body.extend(compile_bind_argument(&parameter.pattern));
    }
    if let Some(rest_parameter) = &parameters.rest {
        compiled_body.extend(compile_bind_rest_argument(&rest_parameter));
    }

    subfunctions
        .into_iter()
        .chain(std::iter::once(Mcfunction {
            name: function_name,
            body: compiled_body,
        }))
        .collect()
}

fn compile_init_function() -> Mcfunction {
    Mcfunction {
        name: String::from("smelter_init.mcfunction"),
        body: vec![
            String::from("data modify storage smelter environment_stack set value []"),
            String::from("data modify storage smelter current_arguments set value []"),
            String::from(
                "data modify storage smelter current_environment set value {parent: '', bindings: {}}",
            ),
            String::from("data modify storage smelter current_return_value set value {}"),
        ],
    }
}

fn compile_bind_argument(pattern: &BindingPattern) -> Vec<String> {
    match &pattern.kind {
        BindingPatternKind::BindingIdentifier(bi) => vec![
            format!(
                "execute unless data storage smelter current_arguments[0] run data modify storage smelter current_environment.bindings.{} set value {{type: 'undefined'}}",
                bi.name.as_str(),
            ),
            format!(
                "execute if data storage smelter current_arguments[0] run data modify storage smelter current_environment.bindings.{} set from storage smelter current_arguments[0]",
                bi.name.as_str(),
            ),
            String::from("data remove storage smelter current_arguments[0]"),
        ],
        _ => Vec::new(),
    }
}

fn compile_bind_rest_argument(pattern: &BindingRestElement) -> Vec<String> {
    let name = pattern
        .argument
        .get_identifier_name()
        .map_or(String::from(""), |atom| atom.into_string());
    vec![format!(
        "data modify storage smelter current_environment.bindings.{name} set from storage smelter current_arguments",
    )]
}
