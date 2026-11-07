use std::{
    cell::RefCell,
    collections::HashMap,
    fmt, fs,
    io::{self, Write},
    rc::Rc,
};
use veyra_bytecode::{Bytecode, Constant, Instruction, SpannedInstruction, validate};
use veyra_source::Span;

#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Unit,
    Array(Rc<RefCell<Vec<Value>>>),
    Struct {
        name: String,
        fields: Rc<RefCell<HashMap<String, Value>>>,
    },
    Range(i64, i64),
    Function(String),
    Builtin(String),
    Module(String),
}
impl PartialEq for Value {
    fn eq(&self, o: &Self) -> bool {
        match (self, o) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Unit, Self::Unit) => true,
            (Self::Array(a), Self::Array(b)) => *a.borrow() == *b.borrow(),
            (
                Self::Struct {
                    name: a,
                    fields: af,
                },
                Self::Struct {
                    name: b,
                    fields: bf,
                },
            ) => a == b && *af.borrow() == *bf.borrow(),
            (Self::Range(a, b), Self::Range(c, d)) => a == c && b == d,
            _ => false,
        }
    }
}
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(v) => write!(f, "{v}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Bool(v) => write!(f, "{v}"),
            Self::String(v) => f.write_str(v),
            Self::Unit => f.write_str("()"),
            Self::Array(v) => {
                f.write_str("[")?;
                for (i, x) in v.borrow().iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{x}")?;
                }
                f.write_str("]")
            }
            Self::Struct { name, .. } => write!(f, "<{name} value>"),
            Self::Range(a, b) => write!(f, "{a}..{b}"),
            Self::Function(v) | Self::Builtin(v) => write!(f, "<fn {v}>"),
            Self::Module(v) => write!(f, "<module {v}>"),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VmError {
    pub message: String,
    pub span: Span,
}
impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for VmError {}
#[derive(Clone)]
struct Binding {
    value: Value,
    mutable: bool,
}
struct LoopState {
    binding: String,
    current: i64,
    end: i64,
    scope_index: usize,
}
struct Frame {
    name: Option<String>,
    module: String,
    ip: usize,
    base: usize,
    scopes: Vec<HashMap<String, Binding>>,
    loops: Vec<LoopState>,
}

pub struct Vm {
    bytecode: Bytecode,
    stack: Vec<Value>,
    frames: Vec<Frame>,
    globals: HashMap<String, Binding>,
    output: Vec<String>,
}
impl Vm {
    /// Creates a VM after validating bytecode structure and version.
    /// # Errors
    /// Returns an error when the bytecode fails validation.
    pub fn new(bytecode: Bytecode) -> Result<Self, VmError> {
        validate(&bytecode)
            .map_err(|m| fault(m, Span::default()))
            .map(|()| Self {
                bytecode,
                stack: Vec::new(),
                frames: Vec::new(),
                globals: HashMap::new(),
                output: Vec::new(),
            })
    }
    #[must_use]
    pub fn output(&self) -> &[String] {
        &self.output
    }
    #[must_use]
    pub fn take_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.output)
    }
    /// Executes initializers followed by `main`.
    /// # Errors
    /// Returns a checked VM runtime fault.
    pub fn run(&mut self) -> Result<Value, VmError> {
        if !self.bytecode.initializer.is_empty() {
            self.frames.push(Frame {
                name: None,
                module: String::new(),
                ip: 0,
                base: 0,
                scopes: vec![HashMap::new()],
                loops: Vec::new(),
            });
            self.execute()?;
            self.stack.clear();
        }
        self.push_call("main", Vec::new(), Span::default())?;
        self.execute()
    }
    fn code(&self) -> &[SpannedInstruction] {
        let frame = self.frames.last().unwrap();
        if let Some(name) = &frame.name {
            &self.bytecode.functions[name].code
        } else {
            &self.bytecode.initializer
        }
    }
    fn execute(&mut self) -> Result<Value, VmError> {
        loop {
            if self.frames.is_empty() {
                return Ok(self.stack.pop().unwrap_or(Value::Unit));
            }
            if self.frames.last().unwrap().ip >= self.code().len() {
                self.return_value(Value::Unit);
                if self.frames.is_empty() {
                    return Ok(self.stack.pop().unwrap_or(Value::Unit));
                }
                continue;
            }
            let item = self.code()[self.frames.last().unwrap().ip].clone();
            self.frames.last_mut().unwrap().ip += 1;
            if let Some(done) = self.step(item)? {
                return Ok(done);
            }
        }
    }
    fn step(&mut self, item: SpannedInstruction) -> Result<Option<Value>, VmError> {
        let span = item.span;
        match item.instruction {
            Instruction::Constant(c) => self.stack.push(constant(c)),
            Instruction::Load(n) => {
                let v = self.load(&n, span)?;
                self.stack.push(v);
            }
            Instruction::Define(n, m) => {
                let v = self.pop(span)?;
                let f = self.frames.last_mut().unwrap();
                if f.name.is_none() {
                    self.globals.insert(
                        n,
                        Binding {
                            value: v,
                            mutable: m,
                        },
                    );
                } else {
                    f.scopes.last_mut().unwrap().insert(
                        n,
                        Binding {
                            value: v,
                            mutable: m,
                        },
                    );
                }
            }
            Instruction::Store(n) => {
                let v = self.pop(span)?;
                self.store(&n, v, span)?;
            }
            Instruction::Pop => {
                self.pop(span)?;
            }
            Instruction::EnterScope => self.frames.last_mut().unwrap().scopes.push(HashMap::new()),
            Instruction::ExitScope => {
                if self.frames.last().unwrap().scopes.len() > 1 {
                    self.frames.last_mut().unwrap().scopes.pop();
                }
            }
            Instruction::Negate => {
                let v = self.pop(span)?;
                self.stack.push(match v {
                    Value::Int(v) => Value::Int(
                        v.checked_neg()
                            .ok_or_else(|| fault("integer overflow", span))?,
                    ),
                    Value::Float(v) => Value::Float(-v),
                    _ => return Err(fault("invalid unary operand", span)),
                });
            }
            Instruction::Not => {
                let v = self.pop(span)?;
                let Value::Bool(v) = v else {
                    return Err(fault("invalid unary operand", span));
                };
                self.stack.push(Value::Bool(!v));
            }
            Instruction::Add => self.binary(span, Op::Add)?,
            Instruction::Subtract => self.binary(span, Op::Sub)?,
            Instruction::Multiply => self.binary(span, Op::Mul)?,
            Instruction::Divide => self.binary(span, Op::Div)?,
            Instruction::Remainder => self.binary(span, Op::Rem)?,
            Instruction::Equal => self.binary(span, Op::Eq)?,
            Instruction::NotEqual => self.binary(span, Op::Ne)?,
            Instruction::Less => self.binary(span, Op::Lt)?,
            Instruction::LessEqual => self.binary(span, Op::Le)?,
            Instruction::Greater => self.binary(span, Op::Gt)?,
            Instruction::GreaterEqual => self.binary(span, Op::Ge)?,
            Instruction::And => self.binary(span, Op::And)?,
            Instruction::Or => self.binary(span, Op::Or)?,
            Instruction::Range => self.binary(span, Op::Range)?,
            Instruction::JumpIfFalse(target) => {
                let v = self.pop(span)?;
                let Value::Bool(v) = v else {
                    return Err(fault("jump condition is not bool", span));
                };
                if !v {
                    self.frames.last_mut().unwrap().ip = target;
                }
            }
            Instruction::Jump(target) => self.frames.last_mut().unwrap().ip = target,
            Instruction::Call(count) => {
                let split = self
                    .stack
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| fault("stack underflow in call", span))?;
                let args = self.stack.split_off(split);
                let callee = self.pop(span)?;
                match callee {
                    Value::Function(n) => self.push_call(&n, args, span)?,
                    Value::Builtin(n) => {
                        let v = self.builtin(&n, args, span)?;
                        self.stack.push(v);
                    }
                    _ => return Err(fault("value is not callable", span)),
                }
            }
            Instruction::Return => {
                let value = self.pop(span)?;
                self.return_value(value);
                if self.frames.is_empty() {
                    return Ok(Some(self.stack.pop().unwrap_or(Value::Unit)));
                }
            }
            Instruction::MakeArray(n) => {
                let start = self
                    .stack
                    .len()
                    .checked_sub(n)
                    .ok_or_else(|| fault("stack underflow building array", span))?;
                let values = self.stack.split_off(start);
                self.stack.push(Value::Array(Rc::new(RefCell::new(values))));
            }
            Instruction::MakeStruct(name, names) => {
                let start = self
                    .stack
                    .len()
                    .checked_sub(names.len())
                    .ok_or_else(|| fault("stack underflow building struct", span))?;
                let values = self.stack.split_off(start);
                let module = self.frames.last().unwrap().module.clone();
                let qualified = if self.bytecode.structs.contains_key(&qualify(&module, &name)) {
                    qualify(&module, &name)
                } else {
                    name
                };
                self.stack.push(Value::Struct {
                    name: qualified,
                    fields: Rc::new(RefCell::new(names.into_iter().zip(values).collect())),
                });
            }
            Instruction::GetField(name) => {
                let object = self.pop(span)?;
                let value = match object {
                    Value::Struct { fields, .. } => fields
                        .borrow()
                        .get(&name)
                        .cloned()
                        .ok_or_else(|| fault(format!("unknown field `{name}`"), span))?,
                    Value::Module(m) => Value::Function(qualify(&m, &name)),
                    _ => return Err(fault("field access requires struct or module", span)),
                };
                self.stack.push(value);
            }
            Instruction::SetField(name) => {
                let value = self.pop(span)?;
                let object = self.pop(span)?;
                let Value::Struct { fields, .. } = object else {
                    return Err(fault("field assignment requires struct", span));
                };
                if !fields.borrow().contains_key(&name) {
                    return Err(fault(format!("unknown field `{name}`"), span));
                }
                fields.borrow_mut().insert(name, value);
            }
            Instruction::GetIndex => {
                let i = self.pop_index(span)?;
                let object = self.pop(span)?;
                let Value::Array(v) = object else {
                    return Err(fault("indexing requires array", span));
                };
                let result = v
                    .borrow()
                    .get(i)
                    .cloned()
                    .ok_or_else(|| fault("array index out of bounds", span))?;
                self.stack.push(result);
            }
            Instruction::SetIndex => {
                let value = self.pop(span)?;
                let i = self.pop_index(span)?;
                let object = self.pop(span)?;
                let Value::Array(v) = object else {
                    return Err(fault("indexed assignment requires array", span));
                };
                let mut array = v.borrow_mut();
                let Some(slot) = array.get_mut(i) else {
                    return Err(fault("array index out of bounds", span));
                };
                *slot = value;
            }
            Instruction::ForInit { binding, end } => {
                let value = self.pop(span)?;
                let Value::Range(start, stop) = value else {
                    return Err(fault("for loop requires int range", span));
                };
                if start >= stop {
                    self.frames.last_mut().unwrap().ip = end;
                } else {
                    let f = self.frames.last_mut().unwrap();
                    f.scopes.push(HashMap::from([(
                        binding.clone(),
                        Binding {
                            value: Value::Int(start),
                            mutable: false,
                        },
                    )]));
                    f.loops.push(LoopState {
                        binding,
                        current: start,
                        end: stop,
                        scope_index: f.scopes.len() - 1,
                    });
                }
            }
            Instruction::ForNext { body, end } => {
                let f = self.frames.last_mut().unwrap();
                let Some(state) = f.loops.last_mut() else {
                    return Err(fault("for-loop state missing", span));
                };
                state.current = state
                    .current
                    .checked_add(1)
                    .ok_or_else(|| fault("integer overflow", span))?;
                if state.current < state.end {
                    f.scopes[state.scope_index]
                        .get_mut(&state.binding)
                        .unwrap()
                        .value = Value::Int(state.current);
                    f.ip = body;
                } else {
                    let index = state.scope_index;
                    f.loops.pop();
                    f.scopes.truncate(index);
                    f.ip = end;
                }
            }
        }
        Ok(None)
    }
    fn push_call(&mut self, name: &str, args: Vec<Value>, span: Span) -> Result<(), VmError> {
        let function = self
            .bytecode
            .functions
            .get(name)
            .ok_or_else(|| fault(format!("undefined function `{name}`"), span))?;
        if args.len() != function.params.len() {
            return Err(fault(
                format!(
                    "function `{name}` expects {} arguments, found {}",
                    function.params.len(),
                    args.len()
                ),
                span,
            ));
        }
        let scope = function
            .params
            .iter()
            .cloned()
            .zip(args)
            .map(|(n, v)| {
                (
                    n,
                    Binding {
                        value: v,
                        mutable: false,
                    },
                )
            })
            .collect();
        self.frames.push(Frame {
            name: Some(name.into()),
            module: function.module.clone(),
            ip: 0,
            base: self.stack.len(),
            scopes: vec![scope],
            loops: Vec::new(),
        });
        Ok(())
    }
    fn return_value(&mut self, value: Value) {
        if let Some(frame) = self.frames.pop() {
            self.stack.truncate(frame.base);
            self.stack.push(value);
        }
    }
    fn load(&self, name: &str, span: Span) -> Result<Value, VmError> {
        let frame = self.frames.last().unwrap();
        for s in frame.scopes.iter().rev() {
            if let Some(v) = s.get(name) {
                return Ok(v.value.clone());
            }
        }
        if let Some(v) = self.globals.get(name) {
            return Ok(v.value.clone());
        }
        let qualified = qualify(&frame.module, name);
        if self.bytecode.functions.contains_key(&qualified) {
            return Ok(Value::Function(qualified));
        }
        if self.bytecode.modules.contains(&name.to_owned()) {
            return Ok(Value::Module(name.into()));
        }
        if matches!(
            name,
            "println" | "len" | "input" | "read_file" | "write_file"
        ) {
            return Ok(Value::Builtin(name.into()));
        }
        Err(fault(format!("undefined name `{name}`"), span))
    }
    fn store(&mut self, name: &str, value: Value, span: Span) -> Result<(), VmError> {
        for s in self.frames.last_mut().unwrap().scopes.iter_mut().rev() {
            if let Some(v) = s.get_mut(name) {
                if !v.mutable {
                    return Err(fault(
                        format!("cannot assign to immutable binding `{name}`"),
                        span,
                    ));
                }
                v.value = value;
                return Ok(());
            }
        }
        if let Some(v) = self.globals.get_mut(name) {
            if !v.mutable {
                return Err(fault(
                    format!("cannot assign to immutable binding `{name}`"),
                    span,
                ));
            }
            v.value = value;
            return Ok(());
        }
        Err(fault(format!("undefined binding `{name}`"), span))
    }
    fn pop(&mut self, span: Span) -> Result<Value, VmError> {
        self.stack
            .pop()
            .ok_or_else(|| fault("operand stack underflow", span))
    }
    fn pop_index(&mut self, span: Span) -> Result<usize, VmError> {
        let Value::Int(v) = self.pop(span)? else {
            return Err(fault("array index must be int", span));
        };
        usize::try_from(v).map_err(|_| fault("array index out of bounds", span))
    }
    fn builtin(&mut self, name: &str, args: Vec<Value>, span: Span) -> Result<Value, VmError> {
        match name {
            "println" => {
                if args.len() != 1 {
                    return Err(fault("println expects one argument", span));
                }
                self.output.push(args[0].to_string());
                Ok(Value::Unit)
            }
            "len" => {
                if args.len() != 1 {
                    return Err(fault("len expects one argument", span));
                }
                match &args[0] {
                    Value::Array(v) => Ok(Value::Int(
                        i64::try_from(v.borrow().len()).unwrap_or(i64::MAX),
                    )),
                    Value::String(v) => Ok(Value::Int(
                        i64::try_from(v.chars().count()).unwrap_or(i64::MAX),
                    )),
                    _ => Err(fault("len expects array or string", span)),
                }
            }
            "input" => {
                if args.len() != 1 {
                    return Err(fault("input expects one argument", span));
                }
                let Value::String(prompt) = &args[0] else {
                    return Err(fault("input expects a string prompt", span));
                };
                print!("{prompt}");
                io::stdout()
                    .flush()
                    .map_err(|e| fault(format!("I/O error: {e}"), span))?;
                let mut line = String::new();
                io::stdin()
                    .read_line(&mut line)
                    .map_err(|e| fault(format!("I/O error: {e}"), span))?;
                while line.ends_with(['\n', '\r']) {
                    line.pop();
                }
                Ok(Value::String(line))
            }
            "read_file" => {
                if args.len() != 1 {
                    return Err(fault("read_file expects one argument", span));
                }
                let Value::String(path) = &args[0] else {
                    return Err(fault("read_file expects a string path", span));
                };
                fs::read_to_string(path)
                    .map(Value::String)
                    .map_err(|e| fault(format!("cannot read `{path}`: {e}"), span))
            }
            "write_file" => {
                if args.len() != 2 {
                    return Err(fault("write_file expects two arguments", span));
                }
                let (Value::String(path), Value::String(contents)) = (&args[0], &args[1]) else {
                    return Err(fault("write_file expects string arguments", span));
                };
                fs::write(path, contents)
                    .map_err(|e| fault(format!("cannot write `{path}`: {e}"), span))?;
                Ok(Value::Unit)
            }
            _ => Err(fault(format!("unknown builtin `{name}`"), span)),
        }
    }
    fn binary(&mut self, span: Span, op: Op) -> Result<(), VmError> {
        let b = self.pop(span)?;
        let a = self.pop(span)?;
        let value = match op {
            Op::Eq => Value::Bool(a == b),
            Op::Ne => Value::Bool(a != b),
            Op::Range => match (a, b) {
                (Value::Int(x), Value::Int(y)) => Value::Range(x, y),
                _ => return Err(fault("range requires int", span)),
            },
            Op::And | Op::Or => match (a, b) {
                (Value::Bool(x), Value::Bool(y)) => Value::Bool(if matches!(op, Op::And) {
                    x && y
                } else {
                    x || y
                }),
                _ => return Err(fault("logical operands must be bool", span)),
            },
            Op::Add => match (a, b) {
                (Value::Int(x), Value::Int(y)) => Value::Int(
                    x.checked_add(y)
                        .ok_or_else(|| fault("integer overflow", span))?,
                ),
                (Value::Float(x), Value::Float(y)) => Value::Float(x + y),
                (Value::String(x), Value::String(y)) => Value::String(x + y.as_str()),
                _ => return Err(fault("invalid add operands", span)),
            },
            Op::Sub => number(a, b, span, i64::checked_sub, |x, y| x - y)?,
            Op::Mul => number(a, b, span, i64::checked_mul, |x, y| x * y)?,
            Op::Div => match (a, b) {
                (_, Value::Int(0)) => return Err(fault("division by zero", span)),
                (Value::Int(x), Value::Int(y)) => Value::Int(
                    x.checked_div(y)
                        .ok_or_else(|| fault("integer overflow", span))?,
                ),
                (Value::Float(x), Value::Float(y)) => Value::Float(x / y),
                _ => return Err(fault("invalid division operands", span)),
            },
            Op::Rem => match (a, b) {
                (_, Value::Int(0)) => return Err(fault("remainder by zero", span)),
                (Value::Int(x), Value::Int(y)) => Value::Int(
                    x.checked_rem(y)
                        .ok_or_else(|| fault("integer overflow", span))?,
                ),
                (Value::Float(x), Value::Float(y)) => Value::Float(x % y),
                _ => return Err(fault("invalid remainder operands", span)),
            },
            Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                let order = match (a, b) {
                    (Value::Int(x), Value::Int(y)) => x.cmp(&y),
                    (Value::Float(x), Value::Float(y)) => x
                        .partial_cmp(&y)
                        .ok_or_else(|| fault("cannot order NaN", span))?,
                    (Value::String(x), Value::String(y)) => x.cmp(&y),
                    _ => return Err(fault("invalid comparison operands", span)),
                };
                Value::Bool(match op {
                    Op::Lt => order.is_lt(),
                    Op::Le => order.is_le(),
                    Op::Gt => order.is_gt(),
                    _ => order.is_ge(),
                })
            }
        };
        self.stack.push(value);
        Ok(())
    }
}
#[derive(Clone, Copy)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Range,
}
fn constant(c: Constant) -> Value {
    match c {
        Constant::Int(v) => Value::Int(v),
        Constant::Float(v) => Value::Float(v),
        Constant::Bool(v) => Value::Bool(v),
        Constant::String(v) => Value::String(v),
        Constant::Unit => Value::Unit,
    }
}
fn number(
    a: Value,
    b: Value,
    span: Span,
    int: fn(i64, i64) -> Option<i64>,
    float: fn(f64, f64) -> f64,
) -> Result<Value, VmError> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(
            int(x, y).ok_or_else(|| fault("integer overflow", span))?,
        )),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(float(x, y))),
        _ => Err(fault("invalid numeric operands", span)),
    }
}
fn qualify(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.into()
    } else {
        format!("{module}.{name}")
    }
}
fn fault(message: impl Into<String>, span: Span) -> VmError {
    VmError {
        message: message.into(),
        span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use veyra_ast::Program;
    use veyra_bytecode::compile;
    use veyra_lexer::lex;
    use veyra_parser::parse_program;
    fn program(s: &str) -> Program {
        parse_program(lex(s).unwrap()).unwrap()
    }
    #[test]
    fn executes_functions_and_loops() {
        let b=compile(&program("fn add(a: int,b: int)->int{return a+b;} fn main(){let mut x=add(1,2);while x<5{x=x+1;}println(x);}"),&HashMap::new()).unwrap();
        let mut vm = Vm::new(b).unwrap();
        vm.run().unwrap();
        assert_eq!(vm.output(), &["5"]);
    }
}
