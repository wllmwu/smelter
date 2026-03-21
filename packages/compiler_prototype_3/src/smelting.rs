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

pub enum SmeltingExpression {
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
    pub body: Vec<SmeltingStatement>,
}

pub struct SmeltingProgram {
    pub functions: Vec<SmeltingFunction>,
}
