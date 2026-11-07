use std::{
    cell::RefCell,
    collections::HashMap,
    fmt, fs,
    io::{self, Write},
    rc::Rc,
};
use veyra_ast::*;
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
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Unit, Self::Unit) => true,
            (Self::Array(a), Self::Array(b)) => *a.borrow() == *b.borrow(),
            (
                Self::Struct {
                    name: an,
                    fields: a,
                },
                Self::Struct {
                    name: bn,
                    fields: b,
                },
            ) => an == bn && *a.borrow() == *b.borrow(),
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
            Self::Struct { name, fields } => {
                write!(f, "{name} {{ ")?;
                let mut pairs: Vec<_> = fields
                    .borrow()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                pairs.sort_by(|a, b| a.0.cmp(&b.0));
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                f.write_str(" }")
            }
            Self::Range(a, b) => write!(f, "{a}..{b}"),
            Self::Function(n) | Self::Builtin(n) => write!(f, "<fn {n}>"),
            Self::Module(n) => write!(f, "<module {n}>"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeError {
    pub message: String,
    pub span: Span,
}
impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for RuntimeError {}

#[derive(Clone)]
struct Binding {
    value: Value,
    mutable: bool,
}
#[derive(Clone)]
struct RuntimeFunction {
    ast: Function,
    module: String,
}

pub struct Interpreter {
    functions: HashMap<String, RuntimeFunction>,
    structs: HashMap<String, StructDecl>,
    globals: HashMap<String, Binding>,
    scopes: Vec<HashMap<String, Binding>>,
    output: Vec<String>,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}
impl Interpreter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            structs: HashMap::new(),
            globals: HashMap::new(),
            scopes: Vec::new(),
            output: Vec::new(),
        }
    }
    #[must_use]
    pub fn output(&self) -> &[String] {
        &self.output
    }
    #[must_use]
    pub fn take_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.output)
    }

    /// Loads a root program and imported module programs, then invokes `main`.
    ///
    /// # Errors
    /// Returns a runtime fault for invalid operations, missing declarations, or a bad entry point.
    pub fn run(
        &mut self,
        program: &Program,
        modules: &HashMap<String, Program>,
    ) -> Result<Value, RuntimeError> {
        for (name, module) in modules {
            self.install(module, name, false)?;
            self.globals.insert(
                name.clone(),
                Binding {
                    value: Value::Module(name.clone()),
                    mutable: false,
                },
            );
        }
        self.install(program, "", true)?;
        self.call_named("main", Vec::new(), Span::default())
    }

    fn install(&mut self, program: &Program, module: &str, root: bool) -> Result<(), RuntimeError> {
        for decl in &program.declarations {
            match decl {
                Decl::Function(function) => {
                    let key = qualify(module, &function.name);
                    self.functions.insert(
                        key,
                        RuntimeFunction {
                            ast: function.clone(),
                            module: module.into(),
                        },
                    );
                }
                Decl::Struct(strukt) => {
                    self.structs
                        .insert(qualify(module, &strukt.name), strukt.clone());
                }
                Decl::Global(_) | Decl::Import(_) | Decl::Policy(_) => {}
            }
        }
        for decl in &program.declarations {
            if let Decl::Global(binding) = decl {
                if !root {
                    return Err(err(
                        "module globals are not supported in Veyra 1.0",
                        binding.span,
                    ));
                }
                let value = self.eval(&binding.initializer, module)?;
                self.globals.insert(
                    binding.name.clone(),
                    Binding {
                        value,
                        mutable: binding.mutable,
                    },
                );
            }
        }
        Ok(())
    }

    fn call_named(
        &mut self,
        name: &str,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let function = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| err(format!("undefined function `{name}`"), span))?;
        if args.len() != function.ast.params.len() {
            return Err(err(
                format!(
                    "function `{}` expects {} arguments, received {}",
                    function.ast.name,
                    function.ast.params.len(),
                    args.len()
                ),
                span,
            ));
        }
        let scope = function
            .ast
            .params
            .iter()
            .zip(args)
            .map(|(p, v)| {
                (
                    p.name.clone(),
                    Binding {
                        value: v,
                        mutable: false,
                    },
                )
            })
            .collect();
        self.scopes.push(scope);
        let result = self.exec_block(&function.ast.body, &function.module);
        self.scopes.pop();
        match result? {
            Flow::Return(v) => Ok(v),
            Flow::Continue => Ok(Value::Unit),
        }
    }

    fn exec_block(&mut self, block: &Block, module: &str) -> Result<Flow, RuntimeError> {
        self.scopes.push(HashMap::new());
        for statement in block {
            let flow = self.exec(statement, module);
            if !matches!(flow, Ok(Flow::Continue)) {
                self.scopes.pop();
                return flow;
            }
        }
        self.scopes.pop();
        Ok(Flow::Continue)
    }

    fn exec(&mut self, statement: &Stmt, module: &str) -> Result<Flow, RuntimeError> {
        match statement {
            Stmt::Let(binding) => {
                let value = self.eval(&binding.initializer, module)?;
                self.define(&binding.name, value, binding.mutable, binding.span)?;
                Ok(Flow::Continue)
            }
            Stmt::Expr(expr) => {
                self.eval(expr, module)?;
                Ok(Flow::Continue)
            }
            Stmt::Assign { target, value, .. } => {
                let value = self.eval(value, module)?;
                self.assign(target, value, module)?;
                Ok(Flow::Continue)
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                if self.boolean(condition, module)? {
                    self.exec_block(then_branch, module)
                } else {
                    self.exec_block(else_branch, module)
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                while self.boolean(condition, module)? {
                    if let Flow::Return(v) = self.exec_block(body, module)? {
                        return Ok(Flow::Return(v));
                    }
                }
                Ok(Flow::Continue)
            }
            Stmt::For {
                name,
                iterable,
                body,
                ..
            } => {
                let Value::Range(start, end) = self.eval(iterable, module)? else {
                    return Err(err("for loop requires an int range", iterable.span));
                };
                for value in start..end {
                    self.scopes.push(HashMap::from([(
                        name.clone(),
                        Binding {
                            value: Value::Int(value),
                            mutable: false,
                        },
                    )]));
                    let flow = self.exec_block(body, module);
                    self.scopes.pop();
                    if let Flow::Return(v) = flow? {
                        return Ok(Flow::Return(v));
                    }
                }
                Ok(Flow::Continue)
            }
            Stmt::Return { value, .. } => Ok(Flow::Return(if let Some(v) = value {
                self.eval(v, module)?
            } else {
                Value::Unit
            })),
        }
    }

    fn eval(&mut self, expr: &Expr, module: &str) -> Result<Value, RuntimeError> {
        match &expr.kind {
            ExprKind::Integer(v) => Ok(Value::Int(*v)),
            ExprKind::Float(v) => Ok(Value::Float(*v)),
            ExprKind::Bool(v) => Ok(Value::Bool(*v)),
            ExprKind::String(v) => Ok(Value::String(v.clone())),
            ExprKind::Unit => Ok(Value::Unit),
            ExprKind::Variable(name) => self.lookup(name, module, expr.span),
            ExprKind::Array(items) => Ok(Value::Array(Rc::new(RefCell::new(
                items
                    .iter()
                    .map(|v| self.eval(v, module))
                    .collect::<Result<_, _>>()?,
            )))),
            ExprKind::Struct { name, fields } => self.make_struct(name, fields, module, expr.span),
            ExprKind::Unary { op, operand } => {
                let value = self.eval(operand, module)?;
                match (op, value) {
                    (UnaryOp::Negate, Value::Int(v)) => v
                        .checked_neg()
                        .map(Value::Int)
                        .ok_or_else(|| err("integer overflow", expr.span)),
                    (UnaryOp::Negate, Value::Float(v)) => Ok(Value::Float(-v)),
                    (UnaryOp::Not, Value::Bool(v)) => Ok(Value::Bool(!v)),
                    _ => Err(err("invalid unary operand", expr.span)),
                }
            }
            ExprKind::Binary { left, op, right } => {
                self.binary(left, *op, right, module, expr.span)
            }
            ExprKind::Call { callee, arguments } => {
                let callable = self.eval(callee, module)?;
                let args = arguments
                    .iter()
                    .map(|v| self.eval(v, module))
                    .collect::<Result<Vec<_>, _>>()?;
                self.call_value(callable, args, expr.span)
            }
            ExprKind::Get { object, name } => match self.eval(object, module)? {
                Value::Struct { fields, .. } => fields
                    .borrow()
                    .get(name)
                    .cloned()
                    .ok_or_else(|| err(format!("unknown field `{name}`"), expr.span)),
                Value::Module(m) => Ok(Value::Function(qualify(&m, name))),
                _ => Err(err("field access requires a struct or module", expr.span)),
            },
            ExprKind::Index { object, index } => {
                let object = self.eval(object, module)?;
                let index = self.eval(index, module)?;
                let Value::Int(i) = index else {
                    return Err(err("array index must be int", index_span(expr)));
                };
                let Ok(i) = usize::try_from(i) else {
                    return Err(err("array index out of bounds", expr.span));
                };
                match object {
                    Value::Array(values) => values
                        .borrow()
                        .get(i)
                        .cloned()
                        .ok_or_else(|| err("array index out of bounds", expr.span)),
                    _ => Err(err("indexing requires an array", expr.span)),
                }
            }
            ExprKind::Private { value, .. } => self.eval(value, module),
        }
    }

    fn binary(
        &mut self,
        left: &Expr,
        op: BinaryOp,
        right: &Expr,
        module: &str,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if op == BinaryOp::And {
            let a = self.boolean(left, module)?;
            return if a {
                Ok(Value::Bool(self.boolean(right, module)?))
            } else {
                Ok(Value::Bool(false))
            };
        }
        if op == BinaryOp::Or {
            let a = self.boolean(left, module)?;
            return if a {
                Ok(Value::Bool(true))
            } else {
                Ok(Value::Bool(self.boolean(right, module)?))
            };
        }
        let a = self.eval(left, module)?;
        let b = self.eval(right, module)?;
        match op {
            BinaryOp::Equal => Ok(Value::Bool(a == b)),
            BinaryOp::NotEqual => Ok(Value::Bool(a != b)),
            BinaryOp::Range => match (a, b) {
                (Value::Int(x), Value::Int(y)) => Ok(Value::Range(x, y)),
                _ => Err(err("range bounds must be int", span)),
            },
            BinaryOp::Add => match (a, b) {
                (Value::Int(x), Value::Int(y)) => checked(x.checked_add(y), span),
                (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x + y)),
                (Value::String(x), Value::String(y)) => Ok(Value::String(x + y.as_str())),
                _ => Err(err(
                    "`+` operands must have matching numeric types or string",
                    span,
                )),
            },
            BinaryOp::Subtract => numeric(a, b, span, i64::checked_sub, |x, y| x - y),
            BinaryOp::Multiply => numeric(a, b, span, i64::checked_mul, |x, y| x * y),
            BinaryOp::Divide => match (a, b) {
                (_, Value::Int(0)) => Err(err("division by zero", right.span)),
                (Value::Int(x), Value::Int(y)) => checked(x.checked_div(y), span),
                (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x / y)),
                _ => Err(err("`/` operands must have matching numeric types", span)),
            },
            BinaryOp::Remainder => match (a, b) {
                (_, Value::Int(0)) => Err(err("remainder by zero", right.span)),
                (Value::Int(x), Value::Int(y)) => checked(x.checked_rem(y), span),
                (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x % y)),
                _ => Err(err("`%` operands must have matching numeric types", span)),
            },
            BinaryOp::Less => compare(a, b, span, |o| o.is_lt()),
            BinaryOp::LessEqual => compare(a, b, span, |o| o.is_le()),
            BinaryOp::Greater => compare(a, b, span, |o| o.is_gt()),
            BinaryOp::GreaterEqual => compare(a, b, span, |o| o.is_ge()),
            BinaryOp::And | BinaryOp::Or => unreachable!(),
        }
    }

    fn call_value(
        &mut self,
        callable: Value,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match callable {
            Value::Function(name) => self.call_named(&name, args, span),
            Value::Builtin(name) => self.builtin(&name, args, span),
            _ => Err(err("value is not callable", span)),
        }
    }
    fn builtin(&mut self, name: &str, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
        match name {
            "println" => {
                if args.len() != 1 {
                    return Err(err("println expects one argument", span));
                }
                self.output.push(args[0].to_string());
                Ok(Value::Unit)
            }
            "len" => {
                if args.len() != 1 {
                    return Err(err("len expects one argument", span));
                }
                match &args[0] {
                    Value::Array(v) => Ok(Value::Int(
                        i64::try_from(v.borrow().len()).unwrap_or(i64::MAX),
                    )),
                    Value::String(v) => Ok(Value::Int(
                        i64::try_from(v.chars().count()).unwrap_or(i64::MAX),
                    )),
                    _ => Err(err("len expects array or string", span)),
                }
            }
            "input" => {
                if args.len() != 1 {
                    return Err(err("input expects one argument", span));
                }
                let Value::String(prompt) = &args[0] else {
                    return Err(err("input expects a string prompt", span));
                };
                print!("{prompt}");
                io::stdout()
                    .flush()
                    .map_err(|e| err(format!("I/O error: {e}"), span))?;
                let mut line = String::new();
                io::stdin()
                    .read_line(&mut line)
                    .map_err(|e| err(format!("I/O error: {e}"), span))?;
                while line.ends_with(['\n', '\r']) {
                    line.pop();
                }
                Ok(Value::String(line))
            }
            "read_file" => {
                if args.len() != 1 {
                    return Err(err("read_file expects one argument", span));
                }
                let Value::String(path) = &args[0] else {
                    return Err(err("read_file expects a string path", span));
                };
                fs::read_to_string(path)
                    .map(Value::String)
                    .map_err(|e| err(format!("cannot read `{path}`: {e}"), span))
            }
            "write_file" => {
                if args.len() != 2 {
                    return Err(err("write_file expects two arguments", span));
                }
                let (Value::String(path), Value::String(contents)) = (&args[0], &args[1]) else {
                    return Err(err("write_file expects string arguments", span));
                };
                fs::write(path, contents)
                    .map_err(|e| err(format!("cannot write `{path}`: {e}"), span))?;
                Ok(Value::Unit)
            }
            _ => Err(err(format!("unknown builtin `{name}`"), span)),
        }
    }
    fn lookup(&self, name: &str, module: &str, span: Span) -> Result<Value, RuntimeError> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Ok(v.value.clone());
            }
        }
        if let Some(v) = self.globals.get(name) {
            return Ok(v.value.clone());
        }
        let qualified = qualify(module, name);
        if self.functions.contains_key(&qualified) {
            return Ok(Value::Function(qualified));
        }
        if matches!(
            name,
            "println" | "len" | "input" | "read_file" | "write_file"
        ) {
            return Ok(Value::Builtin(name.into()));
        }
        Err(err(format!("undefined name `{name}`"), span))
    }
    fn define(
        &mut self,
        name: &str,
        value: Value,
        mutable: bool,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let Some(scope) = self.scopes.last_mut() else {
            return Err(err("internal: no active scope", span));
        };
        if scope.contains_key(name) {
            return Err(err(format!("duplicate binding `{name}`"), span));
        }
        scope.insert(name.into(), Binding { value, mutable });
        Ok(())
    }
    fn assign(&mut self, target: &Expr, value: Value, module: &str) -> Result<(), RuntimeError> {
        match &target.kind {
            ExprKind::Variable(name) => {
                for scope in self.scopes.iter_mut().rev() {
                    if let Some(binding) = scope.get_mut(name) {
                        if !binding.mutable {
                            return Err(err(
                                format!("cannot assign to immutable binding `{name}`"),
                                target.span,
                            ));
                        }
                        binding.value = value;
                        return Ok(());
                    }
                }
                if let Some(binding) = self.globals.get_mut(name) {
                    if !binding.mutable {
                        return Err(err(
                            format!("cannot assign to immutable binding `{name}`"),
                            target.span,
                        ));
                    }
                    binding.value = value;
                    return Ok(());
                }
                Err(err(format!("undefined name `{name}`"), target.span))
            }
            ExprKind::Get { object, name } => match self.eval(object, module)? {
                Value::Struct { fields, .. } => {
                    if !fields.borrow().contains_key(name) {
                        return Err(err(format!("unknown field `{name}`"), target.span));
                    }
                    fields.borrow_mut().insert(name.clone(), value);
                    Ok(())
                }
                _ => Err(err("field assignment requires a struct", target.span)),
            },
            ExprKind::Index { object, index } => {
                let array = self.eval(object, module)?;
                let index = self.eval(index, module)?;
                let (Value::Array(values), Value::Int(i)) = (array, index) else {
                    return Err(err(
                        "indexed assignment requires array and int",
                        target.span,
                    ));
                };
                let Ok(i) = usize::try_from(i) else {
                    return Err(err("array index out of bounds", target.span));
                };
                let mut values = values.borrow_mut();
                let Some(slot) = values.get_mut(i) else {
                    return Err(err("array index out of bounds", target.span));
                };
                *slot = value;
                Ok(())
            }
            _ => Err(err("invalid assignment target", target.span)),
        }
    }
    fn make_struct(
        &mut self,
        name: &str,
        fields: &[(String, Expr)],
        module: &str,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let qualified = if self.structs.contains_key(&qualify(module, name)) {
            qualify(module, name)
        } else {
            name.into()
        };
        let declaration = self
            .structs
            .get(&qualified)
            .cloned()
            .ok_or_else(|| err(format!("undefined struct `{name}`"), span))?;
        let mut values = HashMap::new();
        for (field, expr) in fields {
            if values
                .insert(field.clone(), self.eval(expr, module)?)
                .is_some()
            {
                return Err(err(format!("duplicate field `{field}`"), expr.span));
            }
        }
        for expected in &declaration.fields {
            if !values.contains_key(&expected.name) {
                return Err(err(format!("missing field `{}`", expected.name), span));
            }
        }
        if let Some(extra) = values
            .keys()
            .find(|k| !declaration.fields.iter().any(|f| f.name == **k))
        {
            return Err(err(format!("unknown field `{extra}`"), span));
        }
        Ok(Value::Struct {
            name: qualified,
            fields: Rc::new(RefCell::new(values)),
        })
    }
    fn boolean(&mut self, expr: &Expr, module: &str) -> Result<bool, RuntimeError> {
        match self.eval(expr, module)? {
            Value::Bool(v) => Ok(v),
            _ => Err(err("condition must be bool", expr.span)),
        }
    }
}

enum Flow {
    Continue,
    Return(Value),
}
fn qualify(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.into()
    } else {
        format!("{module}.{name}")
    }
}
fn err(message: impl Into<String>, span: Span) -> RuntimeError {
    RuntimeError {
        message: message.into(),
        span,
    }
}
fn checked(value: Option<i64>, span: Span) -> Result<Value, RuntimeError> {
    value
        .map(Value::Int)
        .ok_or_else(|| err("integer overflow", span))
}
fn numeric(
    a: Value,
    b: Value,
    span: Span,
    int: fn(i64, i64) -> Option<i64>,
    float: fn(f64, f64) -> f64,
) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => checked(int(x, y), span),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(float(x, y))),
        _ => Err(err("numeric operands must have matching types", span)),
    }
}
fn compare(
    a: Value,
    b: Value,
    span: Span,
    predicate: fn(std::cmp::Ordering) -> bool,
) -> Result<Value, RuntimeError> {
    let order = match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(&y),
        (Value::Float(x), Value::Float(y)) => x
            .partial_cmp(&y)
            .ok_or_else(|| err("cannot order NaN", span))?,
        (Value::String(x), Value::String(y)) => x.cmp(&y),
        _ => {
            return Err(err(
                "comparison operands must have matching ordered types",
                span,
            ));
        }
    };
    Ok(Value::Bool(predicate(order)))
}
fn index_span(expr: &Expr) -> Span {
    expr.span
}

/// Evaluates a standalone arithmetic expression for the `eval` command.
///
/// # Errors
/// Returns a runtime fault for an invalid or failing operation.
pub fn evaluate(expression: &Expr) -> Result<i64, RuntimeError> {
    let mut interpreter = Interpreter::new();
    match interpreter.eval(expression, "")? {
        Value::Int(value) => Ok(value),
        _ => Err(err("eval expression did not produce int", expression.span)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use veyra_lexer::lex;
    use veyra_parser::{parse_expression, parse_program};
    fn eval(text: &str) -> Result<i64, RuntimeError> {
        evaluate(&parse_expression(lex(text).unwrap()).unwrap())
    }
    #[test]
    fn arithmetic() {
        assert_eq!(eval("2 + 3 * (4 - 1)").unwrap(), 11);
    }
    #[test]
    fn overflow() {
        assert_eq!(
            eval("9223372036854775807 + 1").unwrap_err().message,
            "integer overflow"
        );
    }
    #[test]
    fn program_execution() {
        let p=parse_program(lex("fn add(a: int, b: int) -> int { return a + b; } fn main() { let mut n = add(2, 3); while n < 7 { n = n + 1; } println(n); }").unwrap()).unwrap();
        let mut i = Interpreter::new();
        i.run(&p, &HashMap::new()).unwrap();
        assert_eq!(i.output(), &["7"]);
    }
    #[test]
    fn arrays_structs() {
        let p=parse_program(lex("struct User { name: string, } fn main() { let a = [1, 2]; let u = User { name: \"Ali\", }; println(u.name); println(a[1]); }").unwrap()).unwrap();
        let mut i = Interpreter::new();
        i.run(&p, &HashMap::new()).unwrap();
        assert_eq!(i.output(), &["Ali", "2"]);
    }
}
