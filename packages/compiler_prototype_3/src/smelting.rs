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

pub enum SmeltingExpression {
    Command(ExpressionId, String),
    FunctionCall(ExpressionId, String, Vec<SmeltingExpression>),
    ListElementAccess(
        ExpressionId,
        Box<SmeltingExpression>,
        Box<SmeltingExpression>,
    ),
    ListInitialization(ExpressionId, Vec<SmeltingExpression>),
    Literal(ExpressionId, Box<SmeltingLiteral>),
    Operation(ExpressionId, Box<SmeltingOperation>),
    StructInitialization(ExpressionId, Vec<(String, SmeltingExpression)>),
    StructMemberAccess(ExpressionId, Box<SmeltingExpression>, String),
    Variable(ExpressionId, String),
}

pub enum SmeltingStatement {
    Assignment(String, Box<SmeltingExpression>),
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
        match self {
            SmeltingExpression::Command(id, command) => todo!(),
            SmeltingExpression::FunctionCall(id, name, arguments) => todo!(),
            SmeltingExpression::ListElementAccess(id, list, index) => todo!(),
            SmeltingExpression::ListInitialization(id, elements) => todo!(),
            SmeltingExpression::Literal(id, literal) => todo!(),
            SmeltingExpression::Operation(id, operation) => todo!(),
            SmeltingExpression::StructInitialization(id, members) => todo!(),
            SmeltingExpression::StructMemberAccess(id, strukt, name) => todo!(),
            SmeltingExpression::Variable(id, name) => todo!(),
        }
    }
}

impl Compile for SmeltingStatement {
    fn compile(self, builder: &mut DataPackBuilder) {
        match self {
            SmeltingStatement::Assignment(variable, value) => todo!(),
            SmeltingStatement::Conditional(condition, true_branch, false_branch) => todo!(),
            SmeltingStatement::Expression(expression) => todo!(),
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
    }
}

impl Compile for SmeltingProgram {
    fn compile(self, builder: &mut DataPackBuilder) {
        for function in self.functions {
            function.compile(builder);
        }
    }
}
