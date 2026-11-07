use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};
use veyra_ast::*;
use veyra_source::Span;

pub const BYTECODE_VERSION: u16 = 1;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Constant {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Unit,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Instruction {
    Constant(Constant),
    Load(String),
    Define(String, bool),
    Store(String),
    Pop,
    EnterScope,
    ExitScope,
    Negate,
    Not,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
    Range,
    JumpIfFalse(usize),
    Jump(usize),
    Call(usize),
    Return,
    MakeArray(usize),
    MakeStruct(String, Vec<String>),
    GetField(String),
    SetField(String),
    GetIndex,
    SetIndex,
    ForInit { binding: String, end: usize },
    ForNext { body: usize, end: usize },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpannedInstruction {
    pub instruction: Instruction,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionCode {
    pub name: String,
    pub module: String,
    pub params: Vec<String>,
    pub code: Vec<SpannedInstruction>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bytecode {
    pub version: u16,
    pub functions: HashMap<String, FunctionCode>,
    pub structs: HashMap<String, Vec<String>>,
    pub modules: Vec<String>,
    pub initializer: Vec<SpannedInstruction>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileError {
    pub message: String,
    pub span: Span,
}
impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for CompileError {}

/// Compiles checked AST modules into Veyra bytecode.
///
/// # Errors
/// Returns an error for an invalid assignment target or unsupported module global.
pub fn compile(
    program: &Program,
    modules: &HashMap<String, Program>,
) -> Result<Bytecode, CompileError> {
    let mut out = Bytecode {
        version: BYTECODE_VERSION,
        functions: HashMap::new(),
        structs: HashMap::new(),
        modules: modules.keys().cloned().collect(),
        initializer: Vec::new(),
    };
    compile_unit(program, "", true, &mut out)?;
    for (name, p) in modules {
        compile_unit(p, name, false, &mut out)?;
    }
    Ok(out)
}
fn compile_unit(
    p: &Program,
    module: &str,
    root: bool,
    out: &mut Bytecode,
) -> Result<(), CompileError> {
    for d in &p.declarations {
        match d {
            Decl::Function(f) => {
                let name = qualify(module, &f.name);
                let mut c = Compiler { code: Vec::new() };
                c.block(&f.body)?;
                c.emit(Instruction::Constant(Constant::Unit), f.span);
                c.emit(Instruction::Return, f.span);
                out.functions.insert(
                    name.clone(),
                    FunctionCode {
                        name,
                        module: module.into(),
                        params: f.params.iter().map(|x| x.name.clone()).collect(),
                        code: c.code,
                    },
                );
            }
            Decl::Struct(s) => {
                out.structs.insert(
                    qualify(module, &s.name),
                    s.fields.iter().map(|f| f.name.clone()).collect(),
                );
            }
            Decl::Global(v) if root => {
                let mut c = Compiler {
                    code: std::mem::take(&mut out.initializer),
                };
                c.expr(&v.initializer)?;
                c.emit(Instruction::Define(v.name.clone(), v.mutable), v.span);
                out.initializer = c.code;
            }
            Decl::Global(v) => {
                return Err(CompileError {
                    message: "module globals are not supported".into(),
                    span: v.span,
                });
            }
            Decl::Import(_) | Decl::Policy(_) => {}
        }
    }
    Ok(())
}

struct Compiler {
    code: Vec<SpannedInstruction>,
}
impl Compiler {
    fn emit(&mut self, instruction: Instruction, span: Span) -> usize {
        let i = self.code.len();
        self.code.push(SpannedInstruction { instruction, span });
        i
    }
    fn patch_jump(&mut self, index: usize, target: usize) {
        match &mut self.code[index].instruction {
            Instruction::JumpIfFalse(v) | Instruction::Jump(v) => *v = target,
            Instruction::ForInit { end, .. } => *end = target,
            Instruction::ForNext { end, .. } => *end = target,
            _ => {}
        }
    }
    fn block(&mut self, b: &Block) -> Result<(), CompileError> {
        self.emit(Instruction::EnterScope, Span::default());
        for s in b {
            self.statement(s)?;
        }
        self.emit(Instruction::ExitScope, Span::default());
        Ok(())
    }
    fn statement(&mut self, s: &Stmt) -> Result<(), CompileError> {
        match s {
            Stmt::Let(v) => {
                self.expr(&v.initializer)?;
                self.emit(Instruction::Define(v.name.clone(), v.mutable), v.span);
            }
            Stmt::Expr(e) => {
                self.expr(e)?;
                self.emit(Instruction::Pop, e.span);
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => match &target.kind {
                ExprKind::Variable(n) => {
                    self.expr(value)?;
                    self.emit(Instruction::Store(n.clone()), *span);
                }
                ExprKind::Get { object, name } => {
                    self.expr(object)?;
                    self.expr(value)?;
                    self.emit(Instruction::SetField(name.clone()), *span);
                }
                ExprKind::Index { object, index } => {
                    self.expr(object)?;
                    self.expr(index)?;
                    self.expr(value)?;
                    self.emit(Instruction::SetIndex, *span);
                }
                _ => {
                    return Err(CompileError {
                        message: "invalid assignment target".into(),
                        span: *span,
                    });
                }
            },
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                self.expr(condition)?;
                let otherwise = self.emit(Instruction::JumpIfFalse(usize::MAX), condition.span);
                self.block(then_branch)?;
                let done = self.emit(Instruction::Jump(usize::MAX), *span);
                self.patch_jump(otherwise, self.code.len());
                self.block(else_branch)?;
                self.patch_jump(done, self.code.len());
            }
            Stmt::While {
                condition,
                body,
                span,
            } => {
                let start = self.code.len();
                self.expr(condition)?;
                let done = self.emit(Instruction::JumpIfFalse(usize::MAX), condition.span);
                self.block(body)?;
                self.emit(Instruction::Jump(start), *span);
                self.patch_jump(done, self.code.len());
            }
            Stmt::For {
                name,
                iterable,
                body,
                span,
            } => {
                self.expr(iterable)?;
                let init = self.emit(
                    Instruction::ForInit {
                        binding: name.clone(),
                        end: usize::MAX,
                    },
                    *span,
                );
                let body_start = self.code.len();
                self.block(body)?;
                let next = self.emit(
                    Instruction::ForNext {
                        body: body_start,
                        end: usize::MAX,
                    },
                    *span,
                );
                let end = self.code.len();
                self.patch_jump(init, end);
                self.patch_jump(next, end);
            }
            Stmt::Return { value, span } => {
                if let Some(e) = value {
                    self.expr(e)?;
                } else {
                    self.emit(Instruction::Constant(Constant::Unit), *span);
                }
                self.emit(Instruction::Return, *span);
            }
        }
        Ok(())
    }
    fn expr(&mut self, e: &Expr) -> Result<(), CompileError> {
        match &e.kind {
            ExprKind::Integer(v) => {
                self.emit(Instruction::Constant(Constant::Int(*v)), e.span);
            }
            ExprKind::Float(v) => {
                self.emit(Instruction::Constant(Constant::Float(*v)), e.span);
            }
            ExprKind::Bool(v) => {
                self.emit(Instruction::Constant(Constant::Bool(*v)), e.span);
            }
            ExprKind::String(v) => {
                self.emit(Instruction::Constant(Constant::String(v.clone())), e.span);
            }
            ExprKind::Unit => {
                self.emit(Instruction::Constant(Constant::Unit), e.span);
            }
            ExprKind::Variable(n) => {
                self.emit(Instruction::Load(n.clone()), e.span);
            }
            ExprKind::Array(items) => {
                for x in items {
                    self.expr(x)?;
                }
                self.emit(Instruction::MakeArray(items.len()), e.span);
            }
            ExprKind::Struct { name, fields } => {
                for (_, x) in fields {
                    self.expr(x)?;
                }
                self.emit(
                    Instruction::MakeStruct(
                        name.clone(),
                        fields.iter().map(|x| x.0.clone()).collect(),
                    ),
                    e.span,
                );
            }
            ExprKind::Unary { op, operand } => {
                self.expr(operand)?;
                self.emit(
                    match op {
                        UnaryOp::Negate => Instruction::Negate,
                        UnaryOp::Not => Instruction::Not,
                    },
                    e.span,
                );
            }
            ExprKind::Binary {
                left,
                op: BinaryOp::And,
                right,
            } => {
                self.expr(left)?;
                let false_jump = self.emit(Instruction::JumpIfFalse(usize::MAX), left.span);
                self.expr(right)?;
                let done = self.emit(Instruction::Jump(usize::MAX), e.span);
                self.patch_jump(false_jump, self.code.len());
                self.emit(Instruction::Constant(Constant::Bool(false)), e.span);
                self.patch_jump(done, self.code.len());
            }
            ExprKind::Binary {
                left,
                op: BinaryOp::Or,
                right,
            } => {
                self.expr(left)?;
                let evaluate_right = self.emit(Instruction::JumpIfFalse(usize::MAX), left.span);
                self.emit(Instruction::Constant(Constant::Bool(true)), e.span);
                let done = self.emit(Instruction::Jump(usize::MAX), e.span);
                self.patch_jump(evaluate_right, self.code.len());
                self.expr(right)?;
                self.patch_jump(done, self.code.len());
            }
            ExprKind::Binary { left, op, right } => {
                self.expr(left)?;
                self.expr(right)?;
                self.emit(
                    match op {
                        BinaryOp::Add => Instruction::Add,
                        BinaryOp::Subtract => Instruction::Subtract,
                        BinaryOp::Multiply => Instruction::Multiply,
                        BinaryOp::Divide => Instruction::Divide,
                        BinaryOp::Remainder => Instruction::Remainder,
                        BinaryOp::Equal => Instruction::Equal,
                        BinaryOp::NotEqual => Instruction::NotEqual,
                        BinaryOp::Less => Instruction::Less,
                        BinaryOp::LessEqual => Instruction::LessEqual,
                        BinaryOp::Greater => Instruction::Greater,
                        BinaryOp::GreaterEqual => Instruction::GreaterEqual,
                        BinaryOp::Range => Instruction::Range,
                        BinaryOp::And | BinaryOp::Or => unreachable!(),
                    },
                    e.span,
                );
            }
            ExprKind::Call { callee, arguments } => {
                self.expr(callee)?;
                for a in arguments {
                    self.expr(a)?;
                }
                self.emit(Instruction::Call(arguments.len()), e.span);
            }
            ExprKind::Get { object, name } => {
                self.expr(object)?;
                self.emit(Instruction::GetField(name.clone()), e.span);
            }
            ExprKind::Index { object, index } => {
                self.expr(object)?;
                self.expr(index)?;
                self.emit(Instruction::GetIndex, e.span);
            }
            ExprKind::Private { value, .. } => self.expr(value)?,
        }
        Ok(())
    }
}

/// Validates all instruction targets and stack-independent structural invariants.
/// # Errors
/// Returns an explanation when bytecode is malformed.
pub fn validate(b: &Bytecode) -> Result<(), String> {
    if b.version != BYTECODE_VERSION {
        return Err(format!("unsupported bytecode version {}", b.version));
    }
    validate_code(&b.initializer)?;
    for f in b.functions.values() {
        validate_code(&f.code)?;
    }
    Ok(())
}
fn validate_code(code: &[SpannedInstruction]) -> Result<(), String> {
    for (i, x) in code.iter().enumerate() {
        let targets: &[usize] = match &x.instruction {
            Instruction::JumpIfFalse(target) | Instruction::Jump(target) => {
                std::slice::from_ref(target)
            }
            Instruction::ForInit { end, .. } => std::slice::from_ref(end),
            Instruction::ForNext { body, .. } => std::slice::from_ref(body),
            _ => &[],
        };
        if targets.iter().any(|target| *target > code.len()) {
            return Err(format!("instruction {i} jumps outside its function"));
        }
        if let Instruction::ForNext { end, .. } = &x.instruction {
            if *end > code.len() {
                return Err(format!("instruction {i} jumps outside its function"));
            }
        }
    }
    Ok(())
}
#[must_use]
pub fn disassemble(b: &Bytecode) -> String {
    let mut names: Vec<_> = b.functions.keys().collect();
    names.sort();
    let mut out = format!("VEYRA BYTECODE v{}\n", b.version);
    for name in names {
        out.push_str(&format!("\nfn {name}:\n"));
        for (i, x) in b.functions[name].code.iter().enumerate() {
            out.push_str(&format!("  {i:04} {:?}\n", x.instruction));
        }
    }
    out
}

/// Serializes a bytecode program with the `VYBC` magic header.
/// # Errors
/// Returns a serialization error if the bytecode cannot be represented.
pub fn encode(b: &Bytecode) -> Result<Vec<u8>, String> {
    let mut bytes = b"VYBC".to_vec();
    bytes.extend(serde_json::to_vec(b).map_err(|e| e.to_string())?);
    Ok(bytes)
}

/// Decodes and validates a `.vyb` bytecode program.
/// # Errors
/// Returns a format or validation error for arbitrary malformed bytes.
pub fn decode(bytes: &[u8]) -> Result<Bytecode, String> {
    if !bytes.starts_with(b"VYBC") {
        return Err("not a Veyra bytecode file".into());
    }
    let bytecode: Bytecode =
        serde_json::from_slice(&bytes[4..]).map_err(|e| format!("invalid bytecode: {e}"))?;
    validate(&bytecode)?;
    Ok(bytecode)
}
fn qualify(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.into()
    } else {
        format!("{module}.{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use veyra_lexer::lex;
    use veyra_parser::parse_program;
    #[test]
    fn compiles_jumps() {
        let p =
            parse_program(lex("fn main() { let mut x=0; while x < 2 { x = x + 1; } }").unwrap())
                .unwrap();
        let b = compile(&p, &HashMap::new()).unwrap();
        assert!(validate(&b).is_ok());
        assert!(disassemble(&b).contains("JumpIfFalse"));
    }

    #[test]
    fn bytecode_round_trip() {
        let program = parse_program(lex("fn main() { println(42); }").unwrap()).unwrap();
        let bytecode = compile(&program, &HashMap::new()).unwrap();
        assert_eq!(decode(&encode(&bytecode).unwrap()).unwrap(), bytecode);
    }

    #[test]
    fn malformed_bytecode_is_rejected() {
        for bytes in [&b""[..], &b"VYBC{"[..], &b"not bytecode"[..]] {
            assert!(decode(bytes).is_err());
        }
    }
}
