use crate::data_pack::DataPackBuilder;

pub enum SmeltingLiteral {
    Boolean(bool),
    Integer(i32),
    Float(f64),
    String(String),
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

type ExpressionId = i32;

pub struct SmeltingExpression {
    id: ExpressionId,
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
    ListElementAccess(Box<SmeltingExpression>, Box<SmeltingExpression>),
    ListInitialization(Vec<SmeltingExpression>),
    Literal(Box<SmeltingLiteral>),
    Operation(Box<SmeltingOperation>),
    StructInitialization(Vec<(String, SmeltingExpression)>),
    StructMemberAccess(Box<SmeltingExpression>, String),
    Variable(String),
}

pub enum SmeltingAssignmentTarget {
    ListIndex(String, i32),
    ListVariableIndex(String, String),
    StructProperty(String, String),
    Variable(String),
}

pub enum SmeltingStatement {
    Assignment(Box<SmeltingAssignmentTarget>, Box<SmeltingExpression>),
    Conditional(
        Box<SmeltingExpression>,
        Vec<SmeltingStatement>,
        Vec<SmeltingStatement>,
    ),
    Expression(Box<SmeltingExpression>),
    Loop(
        Box<SmeltingStatement>,
        Box<SmeltingExpression>,
        Box<SmeltingStatement>,
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
    fn compile(self, builder: &mut DataPackBuilder);
}

impl Compile for SmeltingExpression {
    fn compile(self, builder: &mut DataPackBuilder) {
        let expression_key = self.get_key();
        match self.kind {
            SmeltingExpressionKind::Command(command) => todo!(),
            SmeltingExpressionKind::FunctionCall(name, arguments) => {
                let mut argument_keys: Vec<String> = Vec::new();
                for argument in arguments {
                    argument_keys.push(argument.get_key());
                    argument.compile(builder);
                }
                builder.push_command(format!(
                    "data modify storage smelter:smelter fnargs set value []"
                ));
                for arg_key in argument_keys {
                    builder.push_command(format!("data modify storage smelter:smelter fnargs append from storage smelter:smelter stack[-1].expressions.{arg_key}"));
                }
                builder.push_command(format!("data modify storage smelter:smelter stack append value {{variables: {{}}, expressions: {{}}}}"));
                builder.push_command(format!("execute store result score #fn_result smelter_internal store success score #fn_success smelter_internal run function smelter:{name}"));
                builder.push_command(format!(
                    "execute if score #fn_success smelter_internal matches 0 run return fail"
                ));
                builder.push_command(format!("data remove storage smelter:smelter stack[-1]"));
                builder.push_command(format!("data modify storage smelter:smelter stack[-1].expressions.{expression_key} set from storage smelter:smelter fnret"));
            }
            SmeltingExpressionKind::ListElementAccess(list, index) => todo!(),
            SmeltingExpressionKind::ListInitialization(elements) => todo!(),
            SmeltingExpressionKind::Literal(literal) => todo!(),
            SmeltingExpressionKind::Operation(operation) => todo!(),
            SmeltingExpressionKind::StructInitialization(members) => todo!(),
            SmeltingExpressionKind::StructMemberAccess(strukt, name) => todo!(),
            SmeltingExpressionKind::Variable(name) => todo!(),
        }
    }
}

impl Compile for SmeltingStatement {
    fn compile(self, builder: &mut DataPackBuilder) {
        match self {
            SmeltingStatement::Assignment(target, value) => todo!(),
            SmeltingStatement::Conditional(condition, true_branch, false_branch) => todo!(),
            SmeltingStatement::Expression(expression) => {
                expression.compile(builder);
            }
            SmeltingStatement::Loop(initialization, condition, update, body) => todo!(),
            SmeltingStatement::Return(value) => todo!(),
            SmeltingStatement::Throw(value) => todo!(),
        }
    }
}

impl Compile for SmeltingFunction {
    fn compile(self, builder: &mut DataPackBuilder) {
        builder.push_function(self.name);

        for (i, parameter) in self.parameters.iter().enumerate() {
            builder.push_command(format!("data modify storage smelter:smelter stack[-1].variables.{parameter} set from storage smelter:smelter fnargs[{i}]"));
        }
        builder.push_command(format!(
            "data modify storage smelter:smelter fnargs set value []"
        ));
        builder.push_command(format!("data remove storage smelter:smelter fnret"));

        for statement in self.body {
            statement.compile(builder);
        }

        builder.pop_function();
    }
}

impl Compile for SmeltingProgram {
    fn compile(self, builder: &mut DataPackBuilder) {
        builder.push_function(String::from("initialize"));
        builder.push_command(format!(
            "data modify storage smelter:smelter stack set value []"
        ));
        builder.push_command(format!("scoreboard objectives add smelter_internal dummy"));
        builder.pop_function();

        for function in self.functions {
            function.compile(builder);
        }
    }
}
