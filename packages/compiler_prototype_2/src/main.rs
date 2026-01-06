use anyhow::{Context, Result, bail};
use clap::Parser as CliParser;
use oxc::{
    allocator::Allocator,
    ast::{
        AstKind,
        ast::{Expression, Function, FunctionType, Program, Statement, VariableDeclarationKind},
    },
    parser::Parser,
    semantic::{AstNodes, ScopeId, Scoping, SemanticBuilder},
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

    let source_type = SourceType::mjs(); // Parse as module to force strict mode
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

    let data_pack = compile_program(
        &program,
        semantic_result.semantic.scoping(),
        semantic_result.semantic.nodes(),
    )?;
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

fn make_function_file_name(function_name: Option<&str>, span: &Span) -> String {
    let number = span.start;
    if let Some(name) = function_name {
        format!("{name}_{number}")
    } else {
        format!("anon_func_{number}")
    }
}

fn compile_program(program: &Program, scoping: &Scoping, nodes: &AstNodes) -> Result<DataPack> {
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
                "$data modify storage smelter:smelter $(output_path) set from storage smelter:smelter heap[$(heap_index)]",
            ],
        ),
        // Abstract operations
        Mcfunction::new_static(
            "absop/macro_create_binding",
            &[],
            &[
                "$data modify storage smelter:smelter heap[$(heap_index)].bindings.$(N) set value $(bound)",
            ],
        ),
        Mcfunction::new_static(
            "absop/create_immutable_binding",
            &["envRec", "N"],
            &[
                "data modify storage smelter:smelter macroargs set value {bound: {}}",
                "data modify storage smelter:smelter macroargs.heap_index set from storage smelter:smelter internal_stack[-1].envRec.record",
                "data modify storage smelter:smelter macroargs.N set from storage smelter:smelter internal_stack[-1].N",
                "function smelter:absop/macro_create_binding with storage smelter:smelter macroargs",
            ],
        ),
        Mcfunction::new_static(
            "absop/create_mutable_binding",
            &["envRec", "N", "D"],
            &[
                "data modify storage smelter:smelter macroargs set value {bound: {mutable: true}}",
                "data modify storage smelter:smelter macroargs.heap_index set from storage smelter:smelter internal_stack[-1].envRec.record",
                "data modify storage smelter:smelter macroargs.N set from storage smelter:smelter internal_stack[-1].N",
                "execute if data storage smelter:smelter internal_stack[-1].D.boolean.true run data modify storage smelter:smelter macroargs.bound.deletable set value true",
                "function smelter:absop/macro_create_binding with storage smelter:smelter macroargs",
            ],
        ),
        Mcfunction::new_static(
            "absop/define_own_property",
            &["O", "P", "Desc"],
            &["function smelter:absop/ordinary_define_own_property"],
        ),
        Mcfunction::new_static(
            "absop/get_identifier_reference",
            &["env", "name"],
            &[
                // If env is null, return UNRESOLVABLE
                // n.b. can do enums with direct string equality comparison in NBT paths e.g. execute if data foo.bar{type: 'baz'} run ... instead of current model using execute if data foo.bar.type.baz run ...
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
            "absop/get_own_property",
            &["O", "P"],
            &[
                "function smelter:absop/ordinary_get_own_property",
                "data modify storage smelter:smelter internal_stack[-1]._result set from storage smelter:smelter fnret",
                "data modify storage smelter:smelter fnret set value {record_type: {CompletionRecord: true}, Type: {NORMAL: true}}",
                "data modify storage smelter:smelter fnret.Value set from storage smelter:smelter internal_stack[-1]._result",
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
                "$execute if data storage smelter:smelter internal_stack[-1]._deref_env.bindings.$(N) run data modify storage smelter:smelter fnret set value {boolean: {true: true}}",
            ],
        ),
        Mcfunction::new_static(
            "absop/initialize_binding",
            &["envRec", "N", "V"],
            &[
                "data modify storage smelter:smelter macroargs set value {}",
                "data modify storage smelter:smelter macroargs.heap_index set from storage smelter:smelter internal_stack[-1].envRec.record",
                "data modify storage smelter:smelter macroargs.N set from storage smelter:smelter internal_stack[-1].N",
                "data modify storage smelter:smelter macroargs.V set from storage smelter:smelter internal_stack[-1].V",
                "function smelter:absop/initialize_binding_macro with storage smelter:smelter macroargs",
            ],
        ),
        Mcfunction::new_static(
            "absop/initialize_binding_macro",
            &[],
            &[
                "$data modify storage smelter:smelter heap[$(heap_index)].bindings.$(N).value set value $(V)",
            ],
        ),
        Mcfunction::new_static(
            "absop/is_extensible",
            &["O"],
            &["function smelter:absop/ordinary_is_extensible"],
        ),
        Mcfunction::new_static(
            "absop/ordinary_define_own_property",
            &["O", "P", "Desc"],
            &[
                // Let current be ?O.[[GetOwnProperty]](P)
                "data modify storage smelter:smelter fnargs set value []",
                "data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].O",
                "data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].P",
                "function smelter:absop/get_own_property",
                "execute unless data storage smelter:smelter fnret.Type.NORMAL run return 1",
                "data modify storage smelter:smelter internal_stack[-1].current set from storage smelter:smelter fnret",
                // Let extensible be ?IsExtensible(O)
                "data modify storage smelter:smelter fnargs set value []",
                "data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].O",
                "function smelter:absop/is_extensible",
                "execute unless data storage smelter:smelter fnret.Type.NORMAL run return 1",
                "data modify storage smelter:smelter internal_stack[-1].extensible set from storage smelter:smelter fnret",
                // Return ValidateAndApplyPropertyDescriptor(O, P, extensible, Desc, current)
                "data modify storage smelter:smelter fnargs set value []",
                "data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].O",
                "data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].P",
                "data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].extensible",
                "data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].Desc",
                "data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].current",
                "function smelter:absop/validate_and_apply_property_descriptor",
                "data modify storage smelter:smelter internal_stack[-1]._result set from storage smelter:smelter fnret",
                "data modify storage smelter:smelter fnret set value {record_type: {CompletionRecord: true}, Type: {NORMAL: true}}",
                "data modify storage smelter:smelter fnret.Value set from storage smelter:smelter internal_stack[-1]._result",
            ],
        ),
        Mcfunction::new_static(
            "absop/ordinary_function_create",
            &[
                "functionPrototype",
                "thisMode",
                "env",
                "privateEnv",
                "fileName", // non-standard
                "numArgs",  // non-standard
            ],
            &[
                // MakeBasicObject and initialize internal slots
                "data modify storage smelter:smelter fnargs set value [{PrivateElements: [], Prototype: {undefined: true}, Extensible: {boolean: {true: true}}, Environment: {undefined: true}, PrivateEnvironment: {undefined: true}, ConstructorKind: {undefined: true}, ThisMode: {undefined: true}, HomeObject: {undefined: true}, Fields: [], PrivateMethods: [], ClassFieldInitializerName: {EMPTY: true}, IsClassConstructor: {false: true}}]",
                "data modify storage smelter:smelter fnargs[0].Prototype set from storage smelter:smelter internal_stack[-1].functionPrototype",
                "execute if data storage smelter:smelter internal_stack[-1].thisMode{LEXICAL-THIS:true} run data modify storage smelter:smelter fnargs[0].ThisMode set value {LEXICAL: true}",
                "execute unless data storage smelter:smelter internal_stack[-1].thisMode{LEXICAL-THIS:true} run data modify storage smelter:smelter fnargs[0].ThisMode set value {STRICT: true}",
                "data modify storage smelter:smelter fnargs[0].Environment set from storage smelter:smelter internal_stack[-1].env",
                "data modify storage smelter:smelter fnargs[0].PrivateEnvironment set from storage smelter:smelter internal_stack[-1].privateEnv",
                "data modify storage smelter:smelter fnargs[0].file_name set from storage smelter:smelter internal_stack[-1].fileName", // non-standard
                "function smelter:syscall/allocate_on_heap",
                "data modify storage smelter:smelter internal_stack[-1].F set value {}",
                "data modify storage smelter:smelter internal_stack[-1].F.object set from storage smelter:smelter fnret.pointer",
                // SetFunctionLength
                "data modify storage smelter:smelter fnargs set value []",
                "data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].F",
                "data modify storage smelter:smelter fnargs append value {string: 'length'}",
                "data modify storage smelter:smelter fnargs append value {Writable: false, Enumerable: false, Configurable: true}",
                "data modify storage smelter:smelter fnargs[2].Value set from storage smelter:smelter internal_stack[-1].numArgs",
                "function smelter:absop/define_own_property",
                // Return new object
                "data modify storage smelter:smelter fnret set value {}",
                "data modify storage smelter:smelter fnret.object set from storage smelter:smelter internal_stack[-1].pointer",
            ],
        ),
        Mcfunction::new_static(
            "absop/ordinary_get_own_property",
            &["O", "P"],
            &[
                "data modify storage smelter:smelter macroargs set value {output_path: 'internal_stack[-1].deref_O'}",
                "data modify storage smelter:smelter macroargs.heap_index set from storage smelter:smelter internal_stack[-1].O.object",
                "function smelter:syscall/macro_dereference_heap with storage smelter:smelter macroargs",
                "data modify storage smelter:smelter macroargs set value {}",
                "data modify storage smelter:smelter macroargs.P set from storage smelter:smelter internal_stack[-1].P.string",
            ],
        ),
        Mcfunction::new_static(
            "absop/ordinary_get_own_property_macro",
            &[],
            &[
                "execute unless data storage smelter:smelter internal_stack[-1].deref_O.properties.$(P) run data modify storage smelter:smelter fnret set value {undefined: true}",
                "$execute if data storage smelter:smelter internal_stack[-1].deref_O.properties.$(P) run data modify storage smelter:smelter fnret set from storage smelter:smelter internal_stack[-1].deref_O.properties.$(P)",
            ],
        ),
        Mcfunction::new_static(
            "absop/ordinary_is_extensible",
            &["O"],
            &[
                "data modify storage smelter:smelter macroargs set value {output_path: 'internal_stack[-1].deref_O'}",
                "data modify storage smelter:smelter macroargs.heap_index set from storage smelter:smelter internal_stack[-1].O.object",
                "function smelter:syscall/macro_dereference_heap with storage smelter:smelter macroargs",
                "data modify storage smelter:smelter internal_stack[-1]._result set from storage smelter:smelter fnret",
                "data modify storage smelter:smelter fnret set value {record_type: {CompletionRecord: true}, Type: {NORMAL: true}}",
                "data modify storage smelter:smelter fnret.Value set from storage smelter:smelter internal_stack[-1].deref_O.Extensible",
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
        Mcfunction::new_static(
            "absop/set_function_name",
            &["F", "name"],
            &[
                "data modify storage smelter:smelter fnargs set value []",
                "data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].F",
                "data modify storage smelter:smelter fnargs append value {string: 'name'}",
                "data modify storage smelter:smelter fnargs append value {Writable: false, Enumerable: false, Configurable: true}",
                "data modify storage smelter:smelter fnargs[2].Value set from storage smelter:smelter internal_stack[-1].name",
                "function smelter:absop/define_own_property",
            ],
        ),
    ]);

    builder
        .push_mcfunction(String::from("main"))
        .push_command(debug_log(String::from("entering main")))
        .push_command(String::from("function smelter:initialize"));

    builder.extend_commands(vec![
        // Create module environment record
        String::from("data modify storage smelter:smelter fnargs set value [{record_type: {ModuleEnvironmentRecord: true}, bindings: {}, OuterEnv: {null: true}}]"),
        String::from("function smelter:syscall/allocate_on_heap"),
        String::from("data modify storage smelter:smelter internal_stack[-1].env set value {}"),
        String::from("data modify storage smelter:smelter internal_stack[-1].env.record set from storage smelter:smelter sysret.pointer"),
        // Create and push execution context
        String::from("data modify storage smelter:smelter execution_stack append value {evaluations: {}}"),
        String::from("data modify storage smelter:smelter execution_stack[0].LexicalEnvironment set from storage smelter:smelter internal_stack[-1].env"),
    ]);

    let hoisted_declarations = get_hoisted_declarations(scoping, nodes, program.scope_id());
    for hoisted in hoisted_declarations {
        let bound_name = hoisted.bound_name;
        // Create binding
        match hoisted.kind {
            HoistedDeclarationKind::Const => {
                builder.extend_commands(vec![
                    // env.CreateImmutableBinding(dn, true) <- assume strict always true
                    String::from("data modify storage smelter:smelter fnargs set value []"),
                    String::from("data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].env"),
                    format!("data modify storage smelter:smelter fnargs append value {{string: '{bound_name}'}}"),
                    String::from("function smelter:absop/create_immutable_binding"),
                ]);
            }
            _ => {
                builder.extend_commands(vec![
                    // env.CreateMutableBinding(dn, false)
                    String::from("data modify storage smelter:smelter fnargs set value []"),
                    String::from("data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].env"),
                    format!("data modify storage smelter:smelter fnargs append value {{string: '{bound_name}'}}"),
                    String::from("data modify storage smelter:smelter fnargs append value {boolean: {false: true}}"),
                    String::from("function smelter:absop/create_mutable_binding"),
                ]);
            }
        }
        // Initialize binding if applicable
        match hoisted.kind {
            HoistedDeclarationKind::Var => {
                builder.extend_commands(vec![
                    // env.InitializeBinding(dn, undefined)
                    String::from("data modify storage smelter:smelter fnargs set value []"),
                    String::from("data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].env"),
                    format!("data modify storage smelter:smelter fnargs append value {{string: '{bound_name}'}}"),
                    String::from("data modify storage smelter:smelter fnargs append value {undefined: true}"),
                    String::from("function smelter:absop/initialize_binding"),
                ]);
            }
            HoistedDeclarationKind::Function(function) => {
                let function_name = function.id.as_ref().map(|bi| bi.name.as_str());
                let file_name = make_function_file_name(function_name, &function.span);
                builder.extend_commands(vec![
                    // InstantiateOrdinaryFunctionObject
                    // - Call OrdinaryFunctionCreate
                    String::from("data modify storage smelter:smelter fnargs set value []"),
                    String::from("data modify storage smelter:smelter fnargs append value {NON-LEXICAL-THIS: true}"),
                    String::from("data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].env"),
                    String::from("data modify storage smelter:smelter fnargs append value {null: true}"),
                    format!("data modify storage smelter:smelter fnargs append value {{string: '{file_name}'}}"), // non-standard
                    format!("data modify storage smelter:smelter fnargs append value {{number: {}d}}", function.params.items.len()), // non-standard
                    String::from("function smelter:absop/ordinary_function_create"),
                    String::from("data modify storage smelter:smelter internal_stack[-1].fo set from storage smelter:smelter fnret"),
                    // - Call SetFunctionName
                    String::from("data modify storage smelter:smelter fnargs set value []"),
                    String::from("data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].fo"),
                    format!("data modify storage smelter:smelter fnargs append value {{string: '{}'}}", function_name.unwrap_or("default")),
                    String::from("function smelter:absop/set_function_name"),
                    // - Call MakeConstructor
                    String::from("data modify storage smelter:smelter fnargs set value []"),
                    String::from("data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].fo"),
                    String::from("function smelter:absop/make_constructor"),
                    // env.InitializeBinding(dn, fo)
                    String::from("data modify storage smelter:smelter fnargs set value []"),
                    String::from("data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].env"),
                    format!("data modify storage smelter:smelter fnargs append value {{string: '{bound_name}'}}"),
                    String::from("data modify storage smelter:smelter fnargs append from storage smelter:smelter internal_stack[-1].fo"),
                    String::from("function smelter:absop/initialize_binding"),
                ]);
            }
            _ => (),
        }
    }

    for statement in &program.body {
        compile_statement(&mut builder, statement).with_context(|| "Couldn't compile program")?;
    }

    builder
        .push_command(String::from(
            "data remove storage smelter:smelter execution_stack[-1]",
        ))
        .push_command(debug_log(String::from("exiting main")))
        .commit_mcfunction();

    Ok(builder.complete())
}

enum HoistedDeclarationKind<'a> {
    Const,
    Let,
    Var,
    Function(&'a Function<'a>),
}

struct HoistedDeclaration<'a> {
    bound_name: &'a str,
    kind: HoistedDeclarationKind<'a>,
}

fn get_hoisted_declarations<'a>(
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    scope_id: ScopeId,
) -> Vec<HoistedDeclaration<'a>> {
    // n.b. not sure which bindings this actually returns, e.g. does it include child scopes?
    scoping
        .get_bindings(scope_id)
        .iter()
        .filter_map(|(name, symbol_id)| {
            let declaration_node_id = scoping.symbol_declaration(*symbol_id);
            let declaration_node = nodes.get_node(declaration_node_id);
            match declaration_node.kind() {
                AstKind::Function(function) => {
                    if let FunctionType::FunctionDeclaration = function.r#type {
                        Some(HoistedDeclaration {
                            bound_name: *name,
                            kind: HoistedDeclarationKind::Function(function),
                        })
                    } else {
                        None
                    }
                }
                AstKind::VariableDeclaration(variable_declaration) => {
                    match variable_declaration.kind {
                        VariableDeclarationKind::Const => Some(HoistedDeclaration {
                            bound_name: *name,
                            kind: HoistedDeclarationKind::Const,
                        }),
                        VariableDeclarationKind::Let => Some(HoistedDeclaration {
                            bound_name: *name,
                            kind: HoistedDeclarationKind::Let,
                        }),
                        VariableDeclarationKind::Var => Some(HoistedDeclaration {
                            bound_name: *name,
                            kind: HoistedDeclarationKind::Var,
                        }),
                        _ => todo!(),
                    }
                }
                _ => todo!(),
            }
        })
        .collect()
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
        Statement::TSEnumDeclaration(_) => bail!("Not supported: TypeScript enums"),
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
        Expression::BooleanLiteral(boolean_literal) => {
            let value = boolean_literal.value;
            builder.extend_commands(vec![
                debug_log(String::from("evaluating boolean literal")),
                format!("data modify storage smelter:smelter execution_stack[-1].evaluations.{expression_id} set value {{boolean: {{{value}: true}}}}"),
                debug_log(String::from("done evaluating boolean literal")),
            ]);
        }
        Expression::NullLiteral(null_literal) => {
            builder.extend_commands(vec![
                debug_log(String::from("evaluating null literal")),
                format!("data modify storage smelter:smelter execution_stack[-1].evaluations.{expression_id} set value {{null: true}}"),
                debug_log(String::from("done evaluating null literal")),
            ]);
        }
        Expression::NumericLiteral(numeric_literal) => {
            let value = numeric_literal.value;
            builder.extend_commands(vec![
                debug_log(String::from("evaluating number literal")),
                format!("data modify storage smelter:smelter execution_stack[-1].evaluations.{expression_id} set value {{number: {value}d}}"),
                debug_log(String::from("done evaluating number literal")),
            ]);
        }
        Expression::ObjectExpression(object_expression) => todo!(),
        Expression::RegExpLiteral(reg_exp_literal) => todo!(),
        Expression::StringLiteral(string_literal) => {
            let value = string_literal.value.as_str();
            builder.extend_commands(vec![
                debug_log(String::from("evaluating string literal")),
                format!("data modify storage smelter:smelter execution_stack[-1].evaluations.{expression_id} set value {{string: '{value}'}}"),
                debug_log(String::from("done evaluating string literal")),
            ]);
        }
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
        Expression::ChainExpression(chain_expression) => todo!(),
        Expression::ConditionalExpression(conditional_expression) => todo!(),
        Expression::YieldExpression(yield_expression) => todo!(),
        // Organization
        Expression::ImportExpression(_) => bail!("Not supported: import expressions"),
        Expression::ParenthesizedExpression(parenthesized_expression) => todo!(),
        Expression::SequenceExpression(sequence_expression) => todo!(),
        // JSX, TypeScript, and other language extensions
        Expression::JSXElement(_) => bail!("Not supported: JSX expressions"),
        Expression::JSXFragment(_) => bail!("Not supported: JSX expressions"),
        Expression::TSAsExpression(ts_as_expression) => {
            compile_expression(builder, &ts_as_expression.expression, expression_id)?;
        }
        Expression::TSInstantiationExpression(ts_instantiation_expression) => {
            compile_expression(
                builder,
                &ts_instantiation_expression.expression,
                expression_id,
            )?;
        }
        Expression::TSNonNullExpression(ts_non_null_expression) => {
            compile_expression(builder, &ts_non_null_expression.expression, expression_id)?;
        }
        Expression::TSSatisfiesExpression(ts_satisfies_expression) => {
            compile_expression(builder, &ts_satisfies_expression.expression, expression_id)?;
        }
        Expression::TSTypeAssertion(ts_type_assertion) => {
            compile_expression(builder, &ts_type_assertion.expression, expression_id)?;
        }
        Expression::V8IntrinsicExpression(_) => bail!("Not supported: V8 intrinsics"),
    }
    Ok(())
}
