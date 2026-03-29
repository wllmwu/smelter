use std::ops::Deref;

use crate::data_pack::DataPackBuilder;

pub enum SmeltingLiteral {
    Boolean(bool),
    Integer(i32),
    Float(f64),
    String(String),
}

impl ToString for SmeltingLiteral {
    fn to_string(&self) -> String {
        match self {
            SmeltingLiteral::Boolean(b) => b.to_string(),
            SmeltingLiteral::Integer(i) => i.to_string(),
            SmeltingLiteral::Float(f) => f.to_string(),
            SmeltingLiteral::String(s) => s.clone(),
        }
    }
}

pub enum SmeltingOperation {
    BooleanNegation(Box<SmeltingExpression>),
    BooleanConjunction(Box<SmeltingExpression>, Box<SmeltingExpression>),
    BooleanDisjunction(Box<SmeltingExpression>, Box<SmeltingExpression>),
    IntegerNegation(Box<SmeltingExpression>),
    IntegerAddition(Box<SmeltingExpression>, Box<SmeltingExpression>),
    IntegerSubtraction(Box<SmeltingExpression>, Box<SmeltingExpression>),
    IntegerMultiplication(Box<SmeltingExpression>, Box<SmeltingExpression>),
    IntegerDivision(Box<SmeltingExpression>, Box<SmeltingExpression>),
    IntegerModulo(Box<SmeltingExpression>, Box<SmeltingExpression>),
    FloatNegation(Box<SmeltingExpression>),
    FloatAddition(Box<SmeltingExpression>, Box<SmeltingExpression>),
    FloatSubtraction(Box<SmeltingExpression>, Box<SmeltingExpression>),
    FloatMultiplication(Box<SmeltingExpression>, Box<SmeltingExpression>),
    FloatDivision(Box<SmeltingExpression>, Box<SmeltingExpression>),
    FloatModulo(Box<SmeltingExpression>, Box<SmeltingExpression>),
    StringConcatenation(Box<SmeltingExpression>, Box<SmeltingExpression>),
}

type NodeId = i32;

pub struct SmeltingExpression {
    id: NodeId,
    kind: SmeltingExpressionKind,
}

impl SmeltingExpression {
    fn get_key(&self) -> String {
        format!("expr_{}", self.id)
    }
}

pub enum SmeltingExpressionKind {
    Command(String),
    FunctionCall(String, Vec<SmeltingExpression>),
    ListAccess(Box<SmeltingExpression>, Box<SmeltingExpression>),
    ListInitialization(Vec<SmeltingExpression>),
    Literal(Box<SmeltingLiteral>),
    MapAccess(Box<SmeltingExpression>, Box<SmeltingExpression>),
    MapInitialization(Vec<(String, SmeltingExpression)>),
    Operation(Box<SmeltingOperation>),
    Variable(String),
}

pub enum SmeltingAssignmentTarget {
    ListIndexDynamic(Box<SmeltingExpression>, Box<SmeltingExpression>),
    ListIndexStatic(Box<SmeltingExpression>, i32),
    MapKeyDynamic(Box<SmeltingExpression>, Box<SmeltingExpression>),
    MapKeyStatic(Box<SmeltingExpression>, String),
    Variable(String),
}

pub struct SmeltingStatement {
    id: NodeId,
    kind: SmeltingStatementKind,
}

pub enum SmeltingStatementKind {
    Assignment(Box<SmeltingAssignmentTarget>, Box<SmeltingExpression>),
    Conditional(
        Box<SmeltingExpression>,
        Vec<SmeltingStatement>,
        Vec<SmeltingStatement>,
    ),
    Expression(Box<SmeltingExpression>),
    Loop(
        Option<Box<SmeltingStatement>>,
        Box<SmeltingExpression>,
        Option<Box<SmeltingStatement>>,
        Vec<SmeltingStatement>,
    ),
    Return(Box<SmeltingExpression>),
    Throw(Box<SmeltingExpression>),
}

pub struct SmeltingFunction {
    pub name: String,
    pub parameters: Vec<String>,
    pub body: Vec<SmeltingStatement>,
}

pub struct SmeltingProgram {
    pub functions: Vec<SmeltingFunction>,
}

trait Compile {
    fn compile(&self, builder: &mut DataPackBuilder);
}

impl Compile for SmeltingExpression {
    fn compile(&self, builder: &mut DataPackBuilder) {
        let expression_key = self.get_key();
        match &self.kind {
            SmeltingExpressionKind::Command(command) => {
                builder.push_command(command.clone());
            }
            SmeltingExpressionKind::FunctionCall(name, arguments) => {
                let mut argument_keys: Vec<String> = Vec::new();
                for argument in arguments {
                    argument_keys.push(argument.get_key());
                    argument.compile(builder);
                }

                builder.push_command(format!(
                    "data modify storage smelter:smelter fnargs set value []"
                ));
                for argument_key in argument_keys {
                    builder.push_command(format!("data modify storage smelter:smelter fnargs append from storage smelter:smelter stack[-1].expressions.{argument_key}"));
                }

                builder.push_command(format!("data modify storage smelter:smelter stack append value {{variables: {{}}, expressions: {{}}}}"));
                builder.push_command(format!("execute store result score #fn_result smelter_internal store success score #fn_success smelter_internal run function smelter:{name}"));
                builder.push_command(format!(
                    "execute if score #fn_success smelter_internal matches 0 run return fail"
                ));
                builder.push_command(format!("data remove storage smelter:smelter stack[-1]"));
                builder.push_command(format!("data modify storage smelter:smelter stack[-1].expressions.{expression_key} set from storage smelter:smelter fnret"));
            }
            SmeltingExpressionKind::ListAccess(list, index) => {
                let list_key = list.get_key();
                let index_key = index.get_key();

                list.compile(builder);
                index.compile(builder);

                builder.push_command(format!(
                    "data modify storage smelter:smelter macroargs set value {{}}"
                ));
                builder.push_command(format!("data modify storage smelter:smelter macroargs.pointer set from storage smelter:smelter stack[-1].expressions.{list_key}"));
                builder.push_command(format!("data modify storage smelter:smelter macroargs.index set from storage smelter:smelter stack[-1].expressions.{index_key}"));
                builder.push_command(format!(
                    "function smelter:macro_list_get with storage smelter:smelter macroargs"
                ));
            }
            SmeltingExpressionKind::ListInitialization(elements) => {
                let mut element_keys: Vec<String> = Vec::new();
                for element in elements {
                    element_keys.push(element.get_key());
                    element.compile(builder);
                }

                builder.push_command(format!("function smelter:alloc_list"));
                builder.push_command(format!("data modify storage smelter:smelter stack[-1].expressions.{expression_key} set from storage smelter:smelter fnret"));

                if !element_keys.is_empty() {
                    builder.push_command(format!(
                        "data modify storage smelter:smelter macroargs set value {{}}"
                    ));
                    builder.push_command(format!("data modify storage smelter:smelter macroargs.pointer set from storage smelter:smelter stack[-1].expressions.{expression_key}"));
                }
                for element_key in element_keys {
                    builder.push_command(format!("data modify storage smelter:smelter macroargs.value set from storage smelter:smelter stack[-1].expressions.{element_key}"));
                    builder.push_command(format!(
                        "function smelter:macro_list_append with storage smelter:smelter macroargs"
                    ));
                }
            }
            SmeltingExpressionKind::Literal(literal) => {
                let value = literal.to_string();
                builder.push_command(format!("data modify storage smelter:smelter stack[-1].expressions.{expression_key} set value {value}"));
            }
            SmeltingExpressionKind::MapAccess(map, key) => {
                let map_key = map.get_key();
                let key_expression_key = key.get_key();

                map.compile(builder);
                key.compile(builder);

                builder.push_command(format!(
                    "data modify storage smelter:smelter macroargs set value {{}}"
                ));
                builder.push_command(format!("data modify storage smelter:smelter macroargs.pointer set from storage smelter:smelter stack[-1].expressions.{map_key}"));
                builder.push_command(format!("data modify storage smelter:smelter macroargs.key set from storage smelter:smelter stack[-1].expressions.{key_expression_key}"));
                builder.push_command(format!(
                    "function smelter:macro_map_get with storage smelter:smelter macroargs"
                ));
            }
            SmeltingExpressionKind::MapInitialization(entries) => {
                let mut entry_keys: Vec<(String, String)> = Vec::new();
                for (key, value) in entries {
                    entry_keys.push((key.clone(), value.get_key()));
                    value.compile(builder);
                }

                builder.push_command(format!("function smelter:alloc_map"));
                builder.push_command(format!("data modify storage smelter:smelter stack[-1].expressions.{expression_key} set from storage smelter:smelter fnret"));

                if !entry_keys.is_empty() {
                    builder.push_command(format!(
                        "data modify storage smelter:smelter macroargs set value {{}}"
                    ));
                    builder.push_command(format!("data modify storage smelter:smelter macroargs.pointer set from storage smelter:smelter stack[-1].expressions.{expression_key}"));
                }
                for (key, value_key) in entry_keys {
                    builder.push_command(format!(
                        "data modify storage smelter:smelter macroargs.key set value {key}"
                    ));
                    builder.push_command(format!("data modify storage smelter:smelter macroargs.value set from storage smelter:smelter stack[-1].expressions.{value_key}"));
                    builder.push_command(format!(
                        "function smelter:macro_map_set with storage smelter:smelter macroargs"
                    ));
                }
            }
            SmeltingExpressionKind::Operation(operation) => todo!(),
            SmeltingExpressionKind::Variable(name) => {
                builder.push_command(format!("data modify storage smelter:smelter stack[-1].expressions.{expression_key} set from storage smelter:smelter stack[-1].variables.{name}"));
            }
        }
    }
}

impl Compile for SmeltingStatement {
    fn compile(&self, builder: &mut DataPackBuilder) {
        let statement_id = self.id;
        match &self.kind {
            SmeltingStatementKind::Assignment(target, value) => {
                let value_key = value.get_key();

                value.compile(builder);

                match target.deref() {
                    SmeltingAssignmentTarget::ListIndexDynamic(list, index) => {
                        let index_key = index.get_key();
                        let list_key = list.get_key();

                        index.compile(builder);
                        list.compile(builder);

                        builder.push_command(format!(
                            "data modify storage smelter:smelter macroargs set value {{}}"
                        ));
                        builder.push_command(format!("data modify storage smelter:smelter macroargs.index set from storage smelter:smelter stack[-1].expressions.{index_key}"));
                        builder.push_command(format!("data modify storage smelter:smelter macroargs.pointer set from storage smelter:smelter stack[-1].expressions.{list_key}"));
                        builder.push_command(format!("data modify storage smelter:smelter macroargs.value set from storage smelter:smelter stack[-1].expressions.{value_key}"));
                        builder.push_command(format!(
                            "function smelter:macro_list_set with storage smelter:smelter macroargs"
                        ));
                    }
                    SmeltingAssignmentTarget::ListIndexStatic(list, index) => {
                        let list_key = list.get_key();

                        list.compile(builder);

                        builder.push_command(format!("data modify storage smelter:smelter macroargs set value {{index:{index}}}"));
                        builder.push_command(format!("data modify storage smelter:smelter macroargs.pointer set from storage smelter:smelter stack[-1].expressions.{list_key}"));
                        builder.push_command(format!("data modify storage smelter:smelter macroargs.value set from storage smelter:smelter stack[-1].expressions.{value_key}"));
                        builder.push_command(format!(
                            "function smelter:macro_list_set with storage smelter:smelter macroargs"
                        ));
                    }
                    SmeltingAssignmentTarget::MapKeyDynamic(map, key) => {
                        let key_expression_key = key.get_key();
                        let map_key = map.get_key();

                        key.compile(builder);
                        map.compile(builder);

                        builder.push_command(format!(
                            "data modify storage smelter:smelter macroargs set value {{}}"
                        ));
                        builder.push_command(format!("data modify storage smelter:smelter macroargs.key set from storage smelter:smelter stack[-1].expressions.{key_expression_key}"));
                        builder.push_command(format!("data modify storage smelter:smelter macroargs.pointer set from storage smelter:smelter stack[-1].expressions.{map_key}"));
                        builder.push_command(format!("data modify storage smelter:smelter macroargs.value set from storage smelter:smelter stack[-1].expressions.{value_key}"));
                        builder.push_command(format!(
                            "function smelter:macro_map_set with storage smelter:smelter macroargs"
                        ));
                    }
                    SmeltingAssignmentTarget::MapKeyStatic(map, key) => {
                        let map_key = map.get_key();

                        map.compile(builder);

                        builder.push_command(format!(
                            "data modify storage smelter:smelter macroargs set value {{key:{key}}}"
                        ));
                        builder.push_command(format!("data modify storage smelter:smelter macroargs.pointer set from storage smelter:smelter stack[-1].expressions.{map_key}"));
                        builder.push_command(format!("data modify storage smelter:smelter macroargs.value set from storage smelter:smelter stack[-1].expressions.{value_key}"));
                        builder.push_command(format!(
                            "function smelter:macro_map_set with storage smelter:smelter macroargs"
                        ));
                    }
                    SmeltingAssignmentTarget::Variable(name) => {
                        builder.push_command(format!("data modify storage smelter:smelter stack[-1].variables.{name} set from storage smelter:smelter stack[-1].expressions.{value_key}"));
                    }
                }
            }
            SmeltingStatementKind::Conditional(condition, true_branch, false_branch) => {
                let true_branch_name = format!("ifelse_{statement_id}_true");
                let false_branch_name = format!("ifelse_{statement_id}_false");
                let condition_key = condition.get_key();

                builder.push_function(true_branch_name.clone());
                for statement in true_branch {
                    statement.compile(builder);
                }
                builder.pop_function();

                builder.push_function(false_branch_name.clone());
                for statement in false_branch {
                    statement.compile(builder);
                }
                builder.pop_function();

                condition.compile(builder);
                builder.push_command(format!("execute if data storage smelter:smelter stack[-1].expressions{{{condition_key}:true}} store result score #fn_result smelter_internal store success score #fn_success smelter_internal run function smelter:{true_branch_name}"));
                builder.push_command(format!("execute unless data storage smelter:smelter stack[-1].expressions{{{condition_key}:true}} store result score #fn_result smelter_internal store success score #fn_success smelter_internal run function smelter:{false_branch_name}"));
                builder.push_command(format!(
                    "execute if score #fn_success smelter_internal matches 0 run return fail"
                ));
            }
            SmeltingStatementKind::Expression(expression) => {
                expression.compile(builder);
            }
            SmeltingStatementKind::Loop(initialization, condition, update, body) => {
                let body_name = format!("loop_{statement_id}");
                let condition_key = condition.get_key();

                builder.push_function(body_name.clone());
                for statement in body {
                    statement.compile(builder);
                }
                if let Some(statement) = update {
                    statement.compile(builder);
                }
                condition.compile(builder);
                builder.push_command(format!("execute if data storage smelter:smelter stack[-1].expressions.{{{condition_key}:true}} store result score #fn_result smelter_internal store success score #fn_success smelter_internal run function smelter:{body_name}"));
                builder.pop_function();

                if let Some(statement) = initialization {
                    statement.compile(builder);
                }
                condition.compile(builder);
                builder.push_command(format!("execute if data storage smelter:smelter stack[-1].expressions.{{{condition_key}:true}} store result score #fn_result smelter_internal store success score #fn_success smelter_internal run function smelter:{body_name}"));
                builder.push_command(format!(
                    "execute if score #fn_success smelter_internal matches 0 run return fail"
                ));
            }
            SmeltingStatementKind::Return(value) => {
                let value_key = value.get_key();
                value.compile(builder);
                builder.push_command(format!("data modify storage smelter:smelter fnret set from storage smelter:smelter stack[-1].expressions.{value_key}"));
                builder.push_command(format!("return 1"));
            }
            SmeltingStatementKind::Throw(value) => {
                let value_key = value.get_key();
                value.compile(builder);
                builder.push_command(format!("data modify storage smelter:smelter fnret set from storage smelter:smelter stack[-1].expressions.{value_key}"));
                builder.push_command(format!("return fail"));
            }
        }
    }
}

impl Compile for SmeltingFunction {
    fn compile(&self, builder: &mut DataPackBuilder) {
        builder.push_function(self.name.clone());

        for (i, parameter) in self.parameters.iter().enumerate() {
            builder.push_command(format!("data modify storage smelter:smelter stack[-1].variables.{parameter} set from storage smelter:smelter fnargs[{i}]"));
        }
        builder.push_command(format!(
            "data modify storage smelter:smelter fnargs set value []"
        ));
        builder.push_command(format!("data remove storage smelter:smelter fnret"));

        for statement in &self.body {
            statement.compile(builder);
        }

        builder.pop_function();
    }
}

impl Compile for SmeltingProgram {
    fn compile(&self, builder: &mut DataPackBuilder) {
        builder.push_function(String::from("initialize"));
        builder.push_command(format!(
            "data modify storage smelter:smelter stack set value []"
        ));
        builder.push_command(format!("scoreboard objectives add smelter_internal dummy"));
        builder.pop_function();

        builder.push_function(String::from("alloc_list"));
        builder.push_command(format!("execute store result storage smelter:smelter fnret int 1 run data get storage smelter:smelter heap"));
        builder.push_command(format!(
            "data modify storage smelter:smelter heap append value []"
        ));
        builder.pop_function();

        builder.push_function(String::from("alloc_map"));
        builder.push_command(format!("execute store result storage smelter:smelter fnret int 1 run data get storage smelter:smelter heap"));
        builder.push_command(format!(
            "data modify storage smelter:smelter heap append value {{}}"
        ));
        builder.pop_function();

        builder.push_function(String::from("macro_list_append"));
        builder.push_command(format!(
            "$data modify storage smelter:smelter heap[$(pointer)] append value $(value)"
        ));
        builder.pop_function();

        builder.push_function(String::from("macro_list_get"));
        builder.push_command(format!(
            "$data modify storage smelter:smelter fnret set from storage smelter:smelter heap[$(pointer)][$(index)]"
        ));
        builder.pop_function();

        builder.push_function(String::from("macro_list_set"));
        builder.push_command(format!(
            "$data modify storage smelter:smelter heap[$(pointer)][$(index)] set value $(value)"
        ));
        builder.pop_function();

        builder.push_function(String::from("macro_map_get"));
        builder.push_command(format!(
            "$data modify storage smelter:smelter fnret set from storage smelter:smelter heap[$(pointer)].$(key)"
        ));
        builder.pop_function();

        builder.push_function(String::from("macro_map_set"));
        builder.push_command(format!(
            "$data modify storage smelter:smelter heap[$(pointer)].$(key) set value $(value)"
        ));
        builder.pop_function();

        for function in &self.functions {
            function.compile(builder);
        }
    }
}
