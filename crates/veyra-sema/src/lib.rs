use std::{
    collections::{HashMap, HashSet},
    fmt,
};
use veyra_ast::*;
use veyra_source::Span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemaError {
    pub message: String,
    pub span: Span,
}
impl fmt::Display for SemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for SemaError {}

#[derive(Clone)]
struct Variable {
    ty: Type,
    mutable: bool,
}
#[derive(Clone)]
struct Signature {
    params: Vec<Type>,
    returns: Type,
}
#[derive(Clone)]
enum Resolved {
    Value(Type),
    Function(Signature),
    Module,
    Builtin(String),
}

/// Resolves names and statically type-checks a root program and its modules.
///
/// # Errors
/// Returns all errors found up to the first error in each checked declaration.
pub fn check(program: &Program, modules: &HashMap<String, Program>) -> Result<(), Vec<SemaError>> {
    let mut checker = Checker::new();
    checker.collect(program, "", true);
    for (name, module) in modules {
        checker.collect(module, name, false);
        checker.modules.push(name.clone());
    }
    checker.check_unit(program, "");
    for (name, module) in modules {
        checker.check_unit(module, name);
    }
    if checker.errors.is_empty() {
        let mut flow = PolicyFlowChecker::new(&checker.policies, program, modules);
        flow.check_unit(program, "");
        for (name, module) in modules {
            flow.check_unit(module, name);
        }
        checker.errors.extend(flow.errors);
    }
    if checker.errors.is_empty() {
        Ok(())
    } else {
        Err(checker.errors)
    }
}

struct Checker {
    functions: HashMap<String, Signature>,
    public_functions: HashSet<String>,
    structs: HashMap<String, StructDecl>,
    policies: HashMap<String, PolicyDecl>,
    globals: HashMap<String, Variable>,
    modules: Vec<String>,
    scopes: Vec<HashMap<String, Variable>>,
    errors: Vec<SemaError>,
    current_return: Type,
}
impl Checker {
    fn new() -> Self {
        Self {
            functions: HashMap::new(),
            public_functions: HashSet::new(),
            structs: HashMap::new(),
            policies: HashMap::new(),
            globals: HashMap::new(),
            modules: Vec::new(),
            scopes: Vec::new(),
            errors: Vec::new(),
            current_return: Type::Unit,
        }
    }
    fn collect(&mut self, p: &Program, module: &str, root: bool) {
        for d in &p.declarations {
            match d {
                Decl::Function(f) => {
                    let name = qualify(module, &f.name);
                    if root || f.public {
                        self.public_functions.insert(name.clone());
                    }
                    let sig = Signature {
                        params: f.params.iter().map(|p| p.ty.clone()).collect(),
                        returns: f.return_type.clone(),
                    };
                    if self.functions.insert(name.clone(), sig).is_some() {
                        self.push(format!("duplicate function `{name}`"), f.span);
                    }
                }
                Decl::Struct(s) => {
                    let name = qualify(module, &s.name);
                    if self.structs.insert(name.clone(), s.clone()).is_some() {
                        self.push(format!("duplicate struct `{name}`"), s.span);
                    }
                }
                Decl::Global(v) if root => {
                    let ty = v.annotation.clone().unwrap_or(Type::Unit);
                    if self
                        .globals
                        .insert(
                            v.name.clone(),
                            Variable {
                                ty,
                                mutable: v.mutable,
                            },
                        )
                        .is_some()
                    {
                        self.push(format!("duplicate global `{}`", v.name), v.span);
                    }
                }
                Decl::Policy(policy) => {
                    let name = qualify(module, &policy.name);
                    if self.policies.insert(name.clone(), policy.clone()).is_some() {
                        self.push(format!("duplicate policy `{name}`"), policy.span);
                    }
                }
                _ => {}
            }
        }
    }
    fn check_unit(&mut self, p: &Program, module: &str) {
        if module.is_empty() {
            for declaration in &p.declarations {
                if let Decl::Global(binding) = declaration {
                    if let Ok(ty) = self.expr(&binding.initializer, module) {
                        if let Some(want) = &binding.annotation {
                            self.require(want, &ty, binding.initializer.span);
                        } else if let Some(global) = self.globals.get_mut(&binding.name) {
                            global.ty = ty;
                        }
                    }
                }
            }
        }
        for d in &p.declarations {
            match d {
                Decl::Function(f) => self.function(f, module),
                Decl::Global(_) => {}
                Decl::Import(i) => {
                    if !self.modules.contains(&i.name) {
                        self.push(format!("module `{}` was not loaded", i.name), i.span)
                    }
                }
                Decl::Struct(s) => self.validate_struct(s, module),
                Decl::Policy(_) => {}
            }
        }
    }
    fn validate_struct(&mut self, s: &StructDecl, module: &str) {
        let mut seen = HashMap::new();
        for f in &s.fields {
            if seen.insert(&f.name, ()).is_some() {
                self.push(format!("duplicate field `{}`", f.name), f.span);
            }
            self.valid_type(&f.ty, module, f.span);
        }
    }
    fn function(&mut self, f: &Function, module: &str) {
        self.current_return = f.return_type.clone();
        self.scopes.push(HashMap::new());
        for p in &f.params {
            self.valid_type(&p.ty, module, p.span);
            if self
                .scopes
                .last_mut()
                .unwrap()
                .insert(
                    p.name.clone(),
                    Variable {
                        ty: p.ty.clone(),
                        mutable: false,
                    },
                )
                .is_some()
            {
                self.push(format!("duplicate parameter `{}`", p.name), p.span);
            }
        }
        self.block(&f.body, module);
        if f.return_type != Type::Unit && !returns(&f.body) {
            self.push(
                format!(
                    "function `{}` may exit without returning {}",
                    f.name,
                    type_name(&f.return_type)
                ),
                f.span,
            );
        }
        self.scopes.pop();
    }
    fn block(&mut self, b: &Block, module: &str) {
        self.scopes.push(HashMap::new());
        for s in b {
            self.statement(s, module);
        }
        self.scopes.pop();
    }
    fn statement(&mut self, s: &Stmt, module: &str) {
        match s {
            Stmt::Let(v) => {
                if let Ok(actual) = self.expr(&v.initializer, module) {
                    let ty = if let Some(want) = &v.annotation {
                        self.valid_type(want, module, v.span);
                        self.require(want, &actual, v.initializer.span);
                        want.clone()
                    } else {
                        actual
                    };
                    let scope = self.scopes.last_mut().unwrap();
                    if scope
                        .insert(
                            v.name.clone(),
                            Variable {
                                ty,
                                mutable: v.mutable,
                            },
                        )
                        .is_some()
                    {
                        self.push(format!("duplicate binding `{}`", v.name), v.span);
                    }
                }
            }
            Stmt::Expr(e) => {
                let _ = self.expr(e, module);
            }
            Stmt::Assign { target, value, .. } => {
                let target_ty = self.assignment_target(target, module);
                let value_ty = self.expr(value, module);
                if let (Ok(a), Ok(b)) = (target_ty, value_ty) {
                    self.require(&a, &b, value.span);
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.expect_expr(condition, module, &Type::Bool);
                self.block(then_branch, module);
                self.block(else_branch, module);
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.expect_expr(condition, module, &Type::Bool);
                self.block(body, module);
            }
            Stmt::For {
                name,
                iterable,
                body,
                ..
            } => {
                if let Ok(t) = self.expr(iterable, module) {
                    if t != Type::Named("$range".into()) {
                        self.push("for loop requires an int range", iterable.span);
                    }
                }
                self.scopes.push(HashMap::from([(
                    name.clone(),
                    Variable {
                        ty: Type::Int,
                        mutable: false,
                    },
                )]));
                self.block(body, module);
                self.scopes.pop();
            }
            Stmt::Return { value, span } => {
                let actual = if let Some(e) = value {
                    self.expr(e, module)
                } else {
                    Ok(Type::Unit)
                };
                if let Ok(actual) = actual {
                    let expected = self.current_return.clone();
                    self.require(&expected, &actual, *span);
                }
            }
        }
    }
    fn assignment_target(&mut self, e: &Expr, module: &str) -> Result<Type, ()> {
        match &e.kind {
            ExprKind::Variable(name) => match self.resolve(name, module) {
                Some(Resolved::Value(v)) => {
                    let mutable = self.binding(name).is_some_and(|v| v.mutable);
                    if !mutable {
                        self.push(
                            format!("cannot assign to immutable binding `{name}`"),
                            e.span,
                        );
                        Err(())
                    } else {
                        Ok(v)
                    }
                }
                _ => {
                    self.push(format!("undefined binding `{name}`"), e.span);
                    Err(())
                }
            },
            ExprKind::Get { object, name } => {
                let ty = self.expr(object, module)?;
                self.field(&ty, name, e.span, module)
            }
            ExprKind::Index { object, index } => {
                self.expect_expr(index, module, &Type::Int);
                match self.expr(object, module)? {
                    Type::Array(item) => Ok(*item),
                    _ => {
                        self.push("indexed assignment requires an array", e.span);
                        Err(())
                    }
                }
            }
            _ => {
                self.push("invalid assignment target", e.span);
                Err(())
            }
        }
    }
    fn expr(&mut self, e: &Expr, module: &str) -> Result<Type, ()> {
        match &e.kind {
            ExprKind::Integer(_) => Ok(Type::Int),
            ExprKind::Float(_) => Ok(Type::Float),
            ExprKind::Bool(_) => Ok(Type::Bool),
            ExprKind::String(_) => Ok(Type::String),
            ExprKind::Unit => Ok(Type::Unit),
            ExprKind::Variable(name) => match self.resolve(name, module) {
                Some(Resolved::Value(t)) => Ok(t),
                Some(Resolved::Function(_) | Resolved::Builtin(_) | Resolved::Module) => {
                    Ok(Type::Named(format!("$symbol:{name}")))
                }
                None => {
                    self.push(format!("undefined name `{name}`"), e.span);
                    Err(())
                }
            },
            ExprKind::Array(items) => {
                let Some(first) = items.first() else {
                    self.push("cannot infer type of empty array", e.span);
                    return Err(());
                };
                let ty = self.expr(first, module)?;
                for item in &items[1..] {
                    if let Ok(actual) = self.expr(item, module) {
                        self.require(&ty, &actual, item.span);
                    }
                }
                Ok(Type::Array(Box::new(ty)))
            }
            ExprKind::Struct { name, fields } => self.struct_value(name, fields, module, e.span),
            ExprKind::Unary { op, operand } => {
                let ty = self.expr(operand, module)?;
                match op {
                    UnaryOp::Not => {
                        self.require(&Type::Bool, &ty, operand.span);
                        Ok(Type::Bool)
                    }
                    UnaryOp::Negate if matches!(ty, Type::Int | Type::Float) => Ok(ty),
                    _ => {
                        self.push("unary `-` requires int or float", e.span);
                        Err(())
                    }
                }
            }
            ExprKind::Binary { left, op, right } => self.binary(left, *op, right, module, e.span),
            ExprKind::Call { callee, arguments } => self.call(callee, arguments, module, e.span),
            ExprKind::Get { object, name } => {
                if let ExprKind::Variable(module_name) = &object.kind {
                    if self.modules.contains(module_name) {
                        let qualified = qualify(module_name, name);
                        if !self.public_functions.contains(&qualified) {
                            self.push(
                                format!("module `{module_name}` has no public function `{name}`"),
                                e.span,
                            );
                            return Err(());
                        }
                        return self
                            .functions
                            .get(&qualified)
                            .map(|s| {
                                Type::Named(format!(
                                    "$fn:{}:{}",
                                    qualify(module_name, name),
                                    type_name(&s.returns)
                                ))
                            })
                            .ok_or_else(|| {
                                self.push(
                                    format!(
                                        "module `{module_name}` has no public function `{name}`"
                                    ),
                                    e.span,
                                );
                            });
                    }
                }
                let ty = self.expr(object, module)?;
                self.field(&ty, name, e.span, module)
            }
            ExprKind::Index { object, index } => {
                self.expect_expr(index, module, &Type::Int);
                match self.expr(object, module)? {
                    Type::Array(item) => Ok(*item),
                    _ => {
                        self.push("indexing requires an array", e.span);
                        Err(())
                    }
                }
            }
            ExprKind::Private { policy, value } => {
                let qualified = qualify(module, policy);
                if !self.policies.contains_key(&qualified) && !self.policies.contains_key(policy) {
                    self.push(format!("undefined policy `{policy}`"), e.span);
                    return Err(());
                }
                self.expr(value, module)
            }
        }
    }
    fn binary(
        &mut self,
        l: &Expr,
        op: BinaryOp,
        r: &Expr,
        module: &str,
        span: Span,
    ) -> Result<Type, ()> {
        let a = self.expr(l, module)?;
        let b = self.expr(r, module)?;
        match op {
            BinaryOp::And | BinaryOp::Or => {
                self.require(&Type::Bool, &a, l.span);
                self.require(&Type::Bool, &b, r.span);
                Ok(Type::Bool)
            }
            BinaryOp::Equal | BinaryOp::NotEqual => {
                self.require(&a, &b, r.span);
                Ok(Type::Bool)
            }
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                self.require(&a, &b, r.span);
                if !matches!(a, Type::Int | Type::Float | Type::String) {
                    self.push("comparison requires int, float, or string", span);
                    return Err(());
                }
                Ok(Type::Bool)
            }
            BinaryOp::Range => {
                self.require(&Type::Int, &a, l.span);
                self.require(&Type::Int, &b, r.span);
                Ok(Type::Named("$range".into()))
            }
            BinaryOp::Add if a == Type::String && b == Type::String => Ok(Type::String),
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Remainder => {
                self.require(&a, &b, r.span);
                if !matches!(a, Type::Int | Type::Float) {
                    self.push("arithmetic requires int or float operands", span);
                    return Err(());
                }
                Ok(a)
            }
        }
    }
    fn call(&mut self, callee: &Expr, args: &[Expr], module: &str, span: Span) -> Result<Type, ()> {
        let resolved = match &callee.kind {
            ExprKind::Variable(name) => self.resolve(name, module),
            ExprKind::Get { object, name } => {
                if let ExprKind::Variable(m) = &object.kind {
                    let qualified = qualify(m, name);
                    if self.public_functions.contains(&qualified) {
                        self.functions
                            .get(&qualified)
                            .cloned()
                            .map(Resolved::Function)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        };
        match resolved {
            Some(Resolved::Function(sig)) => {
                if sig.params.len() != args.len() {
                    self.push(
                        format!(
                            "expected {} arguments, found {}",
                            sig.params.len(),
                            args.len()
                        ),
                        span,
                    );
                    return Err(());
                }
                for (arg, want) in args.iter().zip(&sig.params) {
                    if let Ok(actual) = self.expr(arg, module) {
                        self.require(want, &actual, arg.span);
                    }
                }
                Ok(sig.returns)
            }
            Some(Resolved::Builtin(name)) => {
                let expected = if name == "write_file" { 2 } else { 1 };
                if args.len() != expected {
                    self.push(format!("{name} expects {expected} argument(s)"), span);
                    return Err(());
                }
                let types = args
                    .iter()
                    .map(|a| self.expr(a, module))
                    .collect::<Result<Vec<_>, _>>()?;
                if name == "len" && !matches!(types[0], Type::Array(_) | Type::String) {
                    self.push("len expects array or string", args[0].span);
                    return Err(());
                }
                if matches!(name.as_str(), "input" | "read_file") && types[0] != Type::String {
                    self.push(format!("{name} expects a string"), args[0].span);
                    return Err(());
                }
                if name == "write_file" && types.iter().any(|t| *t != Type::String) {
                    self.push("write_file expects string arguments", span);
                    return Err(());
                }
                Ok(match name.as_str() {
                    "len" => Type::Int,
                    "input" | "read_file" => Type::String,
                    _ => Type::Unit,
                })
            }
            _ => {
                self.push("value is not callable", callee.span);
                Err(())
            }
        }
    }
    fn struct_value(
        &mut self,
        name: &str,
        fields: &[(String, Expr)],
        module: &str,
        span: Span,
    ) -> Result<Type, ()> {
        let qualified = if self.structs.contains_key(&qualify(module, name)) {
            qualify(module, name)
        } else {
            name.into()
        };
        let Some(s) = self.structs.get(&qualified).cloned() else {
            self.push(format!("undefined struct `{name}`"), span);
            return Err(());
        };
        for expected in &s.fields {
            let Some((_, value)) = fields.iter().find(|(n, _)| n == &expected.name) else {
                self.push(format!("missing field `{}`", expected.name), span);
                continue;
            };
            if let Ok(actual) = self.expr(value, module) {
                self.require(&expected.ty, &actual, value.span);
            }
        }
        for (name, value) in fields {
            if !s.fields.iter().any(|f| f.name == *name) {
                self.push(format!("unknown field `{name}`"), value.span);
            }
        }
        Ok(Type::Named(qualified))
    }
    fn field(&mut self, ty: &Type, name: &str, span: Span, _module: &str) -> Result<Type, ()> {
        if let Type::Named(struct_name) = ty {
            if let Some(s) = self.structs.get(struct_name) {
                if let Some(field) = s.fields.iter().find(|f| f.name == name) {
                    return Ok(field.ty.clone());
                }
            }
        }
        self.push(
            format!("type {} has no field `{name}`", type_name(ty)),
            span,
        );
        Err(())
    }
    fn resolve(&self, name: &str, module: &str) -> Option<Resolved> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(Resolved::Value(v.ty.clone()));
            }
        }
        if let Some(v) = self.globals.get(name) {
            return Some(Resolved::Value(v.ty.clone()));
        }
        if let Some(v) = self.functions.get(&qualify(module, name)) {
            return Some(Resolved::Function(v.clone()));
        }
        if self.modules.contains(&name.to_owned()) {
            return Some(Resolved::Module);
        }
        if matches!(
            name,
            "println" | "len" | "input" | "read_file" | "write_file"
        ) {
            return Some(Resolved::Builtin(name.into()));
        }
        None
    }
    fn binding(&self, name: &str) -> Option<&Variable> {
        for s in self.scopes.iter().rev() {
            if let Some(v) = s.get(name) {
                return Some(v);
            }
        }
        self.globals.get(name)
    }
    fn expect_expr(&mut self, e: &Expr, module: &str, want: &Type) {
        if let Ok(actual) = self.expr(e, module) {
            self.require(want, &actual, e.span);
        }
    }
    fn require(&mut self, want: &Type, actual: &Type, span: Span) {
        if want != actual {
            self.push(
                format!("expected {}, found {}", type_name(want), type_name(actual)),
                span,
            );
        }
    }
    fn valid_type(&mut self, ty: &Type, module: &str, span: Span) {
        match ty {
            Type::Array(x) => self.valid_type(x, module, span),
            Type::Named(n)
                if !self.structs.contains_key(n)
                    && !self.structs.contains_key(&qualify(module, n)) =>
            {
                self.push(format!("unknown type `{n}`"), span)
            }
            _ => {}
        }
    }
    fn push(&mut self, message: impl Into<String>, span: Span) {
        self.errors.push(SemaError {
            message: message.into(),
            span,
        });
    }
}

#[derive(Clone, Debug, Default)]
struct SecurityLabel {
    origins: Vec<LabelOrigin>,
}

#[derive(Clone, Debug)]
struct LabelOrigin {
    policy: String,
    span: Span,
}

impl SecurityLabel {
    fn join(mut self, other: Self) -> Self {
        for origin in other.origins {
            if !self
                .origins
                .iter()
                .any(|existing| existing.policy == origin.policy)
            {
                self.origins.push(origin);
            }
        }
        self
    }
}

#[allow(dead_code)]
struct FlowChecker<'a> {
    policies: &'a HashMap<String, PolicyDecl>,
    globals: HashMap<String, SecurityLabel>,
    scopes: Vec<HashMap<String, SecurityLabel>>,
    errors: Vec<SemaError>,
}

#[allow(dead_code)]
impl<'a> FlowChecker<'a> {
    fn new(policies: &'a HashMap<String, PolicyDecl>) -> Self {
        Self {
            policies,
            globals: HashMap::new(),
            scopes: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn check_unit(&mut self, program: &Program, module: &str) {
        if module.is_empty() {
            for declaration in &program.declarations {
                if let Decl::Global(binding) = declaration {
                    let label = self.expression(&binding.initializer, module);
                    self.globals.insert(binding.name.clone(), label);
                }
            }
        }
        for declaration in &program.declarations {
            if let Decl::Function(function) = declaration {
                self.scopes.push(HashMap::new());
                for parameter in &function.params {
                    if let Some(scope) = self.scopes.last_mut() {
                        scope.insert(parameter.name.clone(), SecurityLabel::default());
                    }
                }
                self.block(&function.body, module);
                self.scopes.pop();
            }
        }
    }

    fn block(&mut self, block: &Block, module: &str) {
        self.scopes.push(HashMap::new());
        for statement in block {
            self.statement(statement, module);
        }
        self.scopes.pop();
    }

    fn statement(&mut self, statement: &Stmt, module: &str) {
        match statement {
            Stmt::Let(binding) => {
                let label = self.expression(&binding.initializer, module);
                if let Some(scope) = self.scopes.last_mut() {
                    scope.insert(binding.name.clone(), label);
                }
            }
            Stmt::Expr(expression) => {
                self.expression(expression, module);
            }
            Stmt::Assign { target, value, .. } => {
                let label = self.expression(value, module);
                if let Some(name) = assignment_root(target) {
                    self.assign(name, label);
                }
                self.expression(target, module);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.expression(condition, module);
                self.block(then_branch, module);
                self.block(else_branch, module);
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.expression(condition, module);
                self.block(body, module);
            }
            Stmt::For {
                name,
                iterable,
                body,
                ..
            } => {
                self.expression(iterable, module);
                self.scopes
                    .push(HashMap::from([(name.clone(), SecurityLabel::default())]));
                self.block(body, module);
                self.scopes.pop();
            }
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.expression(value, module);
                }
            }
        }
    }

    fn expression(&mut self, expression: &Expr, module: &str) -> SecurityLabel {
        match &expression.kind {
            ExprKind::Integer(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Unit => SecurityLabel::default(),
            ExprKind::Variable(name) => self.lookup(name),
            ExprKind::Array(items) => items.iter().fold(SecurityLabel::default(), |label, item| {
                label.join(self.expression(item, module))
            }),
            ExprKind::Struct { fields, .. } => fields
                .iter()
                .fold(SecurityLabel::default(), |label, (_, value)| {
                    label.join(self.expression(value, module))
                }),
            ExprKind::Unary { operand, .. } => self.expression(operand, module),
            ExprKind::Binary { left, right, .. } => self
                .expression(left, module)
                .join(self.expression(right, module)),
            ExprKind::Call { callee, arguments } => {
                let mut label = self.expression(callee, module);
                for argument in arguments {
                    label = label.join(self.expression(argument, module));
                }
                if matches!(&callee.kind, ExprKind::Variable(name) if name == "println")
                    && !label.origins.is_empty()
                {
                    let policies = label
                        .origins
                        .iter()
                        .map(|origin| {
                            format!(
                                "`{}` (introduced at byte {})",
                                origin.policy, origin.span.start
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    self.errors.push(SemaError { message: format!("information-flow violation: value sent to public console sink `println` is protected by {policies}; the console destination has label Public and cannot receive protected data. Use a future authorized, purpose- and destination-scoped release capability instead of removing the label"), span: expression.span });
                }
                label
            }
            ExprKind::Get { object, .. } => self.expression(object, module),
            ExprKind::Index { object, index } => self
                .expression(object, module)
                .join(self.expression(index, module)),
            ExprKind::Private { policy, value } => {
                let mut label = self.expression(value, module);
                let qualified = if self.policies.contains_key(&qualify(module, policy)) {
                    qualify(module, policy)
                } else {
                    policy.clone()
                };
                label = label.join(SecurityLabel {
                    origins: vec![LabelOrigin {
                        policy: qualified,
                        span: expression.span,
                    }],
                });
                label
            }
        }
    }

    fn lookup(&self, name: &str) -> SecurityLabel {
        for scope in self.scopes.iter().rev() {
            if let Some(label) = scope.get(name) {
                return label.clone();
            }
        }
        self.globals.get(name).cloned().unwrap_or_default()
    }

    fn assign(&mut self, name: &str, label: SecurityLabel) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(existing) = scope.get_mut(name) {
                *existing = existing.clone().join(label);
                return;
            }
        }
        if let Some(existing) = self.globals.get_mut(name) {
            *existing = existing.clone().join(label);
        }
    }
}

struct PolicyFlowChecker<'a> {
    policies: &'a HashMap<String, PolicyDecl>,
    functions: HashMap<String, (Function, String)>,
    active_calls: HashSet<String>,
    globals: HashMap<String, SecurityLabel>,
    scopes: Vec<HashMap<String, SecurityLabel>>,
    return_labels: Vec<SecurityLabel>,
    errors: Vec<SemaError>,
}

impl<'a> PolicyFlowChecker<'a> {
    fn new(
        policies: &'a HashMap<String, PolicyDecl>,
        program: &Program,
        modules: &HashMap<String, Program>,
    ) -> Self {
        let mut functions = HashMap::new();
        collect_flow_functions(&mut functions, program, "");
        for (module, program) in modules {
            collect_flow_functions(&mut functions, program, module);
        }
        Self {
            policies,
            functions,
            active_calls: HashSet::new(),
            globals: HashMap::new(),
            scopes: Vec::new(),
            return_labels: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn check_unit(&mut self, program: &Program, module: &str) {
        if module.is_empty() {
            for declaration in &program.declarations {
                if let Decl::Global(binding) = declaration {
                    let label =
                        self.expression(&binding.initializer, module, &SecurityLabel::default());
                    self.globals.insert(binding.name.clone(), label);
                }
            }
        }
        for declaration in &program.declarations {
            if let Decl::Function(function) = declaration {
                let name = qualify(module, &function.name);
                self.analyze_function(
                    &name,
                    vec![SecurityLabel::default(); function.params.len()],
                    &SecurityLabel::default(),
                );
            }
        }
    }

    fn analyze_function(
        &mut self,
        name: &str,
        arguments: Vec<SecurityLabel>,
        pc: &SecurityLabel,
    ) -> SecurityLabel {
        if self.active_calls.contains(name) {
            return arguments.into_iter().fold(pc.clone(), SecurityLabel::join);
        }
        let Some((function, module)) = self.functions.get(name).cloned() else {
            return arguments
                .into_iter()
                .fold(SecurityLabel::default(), SecurityLabel::join);
        };
        self.active_calls.insert(name.to_owned());
        let parameters = function
            .params
            .iter()
            .zip(arguments)
            .map(|(parameter, label)| (parameter.name.clone(), label))
            .collect();
        self.scopes.push(parameters);
        self.return_labels.push(SecurityLabel::default());
        self.block(&function.body, &module, pc);
        let result = self.return_labels.pop().unwrap_or_default();
        self.scopes.pop();
        self.active_calls.remove(name);
        result
    }

    fn block(&mut self, block: &Block, module: &str, pc: &SecurityLabel) {
        self.scopes.push(HashMap::new());
        for statement in block {
            self.statement(statement, module, pc);
        }
        self.scopes.pop();
    }

    fn statement(&mut self, statement: &Stmt, module: &str, pc: &SecurityLabel) {
        match statement {
            Stmt::Let(binding) => {
                let label = self
                    .expression(&binding.initializer, module, pc)
                    .join(pc.clone());
                if let Some(scope) = self.scopes.last_mut() {
                    scope.insert(binding.name.clone(), label);
                }
            }
            Stmt::Expr(expression) => {
                self.expression(expression, module, pc);
            }
            Stmt::Assign { target, value, .. } => {
                let label = self.expression(value, module, pc).join(pc.clone());
                if let Some(name) = assignment_root(target) {
                    self.assign(name, label);
                }
                self.expression(target, module, pc);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let condition_label = self.expression(condition, module, pc);
                let branch_pc = pc.clone().join(condition_label);
                self.block(then_branch, module, &branch_pc);
                self.block(else_branch, module, &branch_pc);
            }
            Stmt::While {
                condition, body, ..
            } => loop {
                let before = self.environment_signature();
                let condition_label = self.expression(condition, module, pc);
                let body_pc = pc.clone().join(condition_label);
                self.block(body, module, &body_pc);
                if self.environment_signature() == before {
                    break;
                }
            },
            Stmt::For {
                name,
                iterable,
                body,
                ..
            } => {
                let iterable_label = self.expression(iterable, module, pc);
                let body_pc = pc.clone().join(iterable_label.clone());
                self.scopes
                    .push(HashMap::from([(name.clone(), iterable_label)]));
                loop {
                    let before = self.environment_signature();
                    self.block(body, module, &body_pc);
                    if self.environment_signature() == before {
                        break;
                    }
                }
                self.scopes.pop();
            }
            Stmt::Return { value, .. } => {
                let label = value.as_ref().map_or_else(
                    || pc.clone(),
                    |value| self.expression(value, module, pc).join(pc.clone()),
                );
                if let Some(result) = self.return_labels.last_mut() {
                    *result = result.clone().join(label);
                }
            }
        }
    }

    fn expression(&mut self, expression: &Expr, module: &str, pc: &SecurityLabel) -> SecurityLabel {
        match &expression.kind {
            ExprKind::Integer(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Unit => SecurityLabel::default(),
            ExprKind::Variable(name) => self.lookup(name),
            ExprKind::Array(items) => items.iter().fold(SecurityLabel::default(), |label, item| {
                label.join(self.expression(item, module, pc))
            }),
            ExprKind::Struct { fields, .. } => fields
                .iter()
                .fold(SecurityLabel::default(), |label, (_, value)| {
                    label.join(self.expression(value, module, pc))
                }),
            ExprKind::Unary { operand, .. } => self.expression(operand, module, pc),
            ExprKind::Binary { left, op, right } => {
                let label = self
                    .expression(left, module, pc)
                    .join(self.expression(right, module, pc));
                if matches!(op, BinaryOp::Divide | BinaryOp::Remainder) {
                    self.report_violation(
                        "runtime error channel",
                        "an observable division or remainder failure",
                        &label,
                        pc,
                        expression.span,
                    );
                }
                label
            }
            ExprKind::Call { callee, arguments } => {
                let argument_labels = arguments
                    .iter()
                    .map(|argument| self.expression(argument, module, pc))
                    .collect::<Vec<_>>();
                let arguments_label = argument_labels
                    .iter()
                    .cloned()
                    .fold(SecurityLabel::default(), SecurityLabel::join);
                if let Some((sink, destination)) = sink_name(callee) {
                    self.report_violation(sink, destination, &arguments_label, pc, expression.span);
                    return arguments_label.join(pc.clone());
                }
                if let Some(function) = self.resolve_function(callee, module) {
                    return self.analyze_function(&function, argument_labels, pc);
                }
                self.expression(callee, module, pc).join(arguments_label)
            }
            ExprKind::Get { object, .. } => self.expression(object, module, pc),
            ExprKind::Index { object, index } => {
                let label = self
                    .expression(object, module, pc)
                    .join(self.expression(index, module, pc));
                self.report_violation(
                    "runtime error channel",
                    "an observable bounds-check failure",
                    &label,
                    pc,
                    expression.span,
                );
                label
            }
            ExprKind::Private { policy, value } => {
                let mut label = self.expression(value, module, pc);
                let qualified = if self.policies.contains_key(&qualify(module, policy)) {
                    qualify(module, policy)
                } else {
                    policy.clone()
                };
                label = label.join(SecurityLabel {
                    origins: vec![LabelOrigin {
                        policy: qualified,
                        span: expression.span,
                    }],
                });
                label
            }
        }
    }

    fn resolve_function(&self, callee: &Expr, module: &str) -> Option<String> {
        match &callee.kind {
            ExprKind::Variable(name) => {
                let qualified = qualify(module, name);
                self.functions.contains_key(&qualified).then_some(qualified)
            }
            ExprKind::Get { object, name } => {
                if let ExprKind::Variable(module) = &object.kind {
                    let qualified = qualify(module, name);
                    self.functions.contains_key(&qualified).then_some(qualified)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn lookup(&self, name: &str) -> SecurityLabel {
        for scope in self.scopes.iter().rev() {
            if let Some(label) = scope.get(name) {
                return label.clone();
            }
        }
        self.globals.get(name).cloned().unwrap_or_default()
    }

    fn assign(&mut self, name: &str, label: SecurityLabel) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(existing) = scope.get_mut(name) {
                *existing = existing.clone().join(label);
                return;
            }
        }
        if let Some(existing) = self.globals.get_mut(name) {
            *existing = existing.clone().join(label);
        }
    }

    fn report_violation(
        &mut self,
        operation: &str,
        destination: &str,
        data: &SecurityLabel,
        pc: &SecurityLabel,
        span: Span,
    ) {
        let effective = data.clone().join(pc.clone());
        if effective.origins.is_empty() {
            return;
        }
        let policies = effective
            .origins
            .iter()
            .map(|origin| {
                format!(
                    "`{}` (introduced at byte {})",
                    origin.policy, origin.span.start
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let control = if pc.origins.is_empty() {
            ""
        } else {
            " The operation is control-dependent on protected data, so its program-counter label is also protected."
        };
        let message = format!(
            "information-flow violation: {operation} targets {destination}, whose clearance is Public, but the effective value is protected by {policies}.{control} No ordinary function or cast may remove this label; an authorized purpose- and destination-scoped release capability is required"
        );
        if !self
            .errors
            .iter()
            .any(|error| error.span == span && error.message == message)
        {
            self.errors.push(SemaError { message, span });
        }
    }

    fn environment_signature(&self) -> Vec<(String, Vec<String>)> {
        let mut entries = Vec::new();
        for (name, label) in &self.globals {
            entries.push((format!("global:{name}"), policy_names(label)));
        }
        for (index, scope) in self.scopes.iter().enumerate() {
            for (name, label) in scope {
                entries.push((format!("{index}:{name}"), policy_names(label)));
            }
        }
        entries.sort();
        entries
    }
}

fn collect_flow_functions(
    functions: &mut HashMap<String, (Function, String)>,
    program: &Program,
    module: &str,
) {
    for declaration in &program.declarations {
        if let Decl::Function(function) = declaration {
            functions.insert(
                qualify(module, &function.name),
                (function.clone(), module.to_owned()),
            );
        }
    }
}

fn sink_name(callee: &Expr) -> Option<(&'static str, &'static str)> {
    match &callee.kind {
        ExprKind::Variable(name) if name == "println" => {
            Some(("console output `println`", "the public console"))
        }
        ExprKind::Variable(name) if name == "write_file" => Some((
            "file output `write_file`",
            "an unprotected file destination",
        )),
        _ => None,
    }
}

fn policy_names(label: &SecurityLabel) -> Vec<String> {
    let mut policies = label
        .origins
        .iter()
        .map(|origin| origin.policy.clone())
        .collect::<Vec<_>>();
    policies.sort();
    policies
}

fn assignment_root(expression: &Expr) -> Option<&str> {
    match &expression.kind {
        ExprKind::Variable(name) => Some(name),
        ExprKind::Get { object, .. } | ExprKind::Index { object, .. } => assignment_root(object),
        _ => None,
    }
}

fn qualify(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.into()
    } else {
        format!("{module}.{name}")
    }
}
fn type_name(t: &Type) -> String {
    match t {
        Type::Int => "int".into(),
        Type::Float => "float".into(),
        Type::Bool => "bool".into(),
        Type::String => "string".into(),
        Type::Unit => "unit".into(),
        Type::Array(x) => format!("[{}]", type_name(x)),
        Type::Named(n) => n.clone(),
    }
}
fn returns(block: &Block) -> bool {
    for s in block {
        match s {
            Stmt::Return { .. } => return true,
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } if !else_branch.is_empty() && returns(then_branch) && returns(else_branch) => {
                return true;
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use veyra_lexer::lex;
    use veyra_parser::parse_program;
    #[test]
    fn valid_program() {
        let p = parse_program(
            lex(
                "fn add(a: int, b: int) -> int { return a + b; } fn main() { println(add(1, 2)); }",
            )
            .unwrap(),
        )
        .unwrap();
        assert!(check(&p, &HashMap::new()).is_ok());
    }
    #[test]
    fn catches_immutable_assignment() {
        let p = parse_program(lex("fn main() { let x = 1; x = 2; }").unwrap()).unwrap();
        assert!(
            check(&p, &HashMap::new()).unwrap_err()[0]
                .message
                .contains("immutable")
        );
    }
    #[test]
    fn catches_bad_return() {
        let p =
            parse_program(lex("fn f() -> int { return true; } fn main() {} ").unwrap()).unwrap();
        assert!(check(&p, &HashMap::new()).is_err());
    }

    #[test]
    fn rejects_direct_and_derived_protected_console_output() {
        let source = "policy HospitalPrivate { owner: \"hospital-a\"; export: deny; } fn main() { let secret = private<HospitalPrivate>(100); println(secret); let calculated = secret + 20; println(calculated); }";
        let program = parse_program(lex(source).unwrap()).unwrap();
        let errors = check(&program, &HashMap::new()).unwrap_err();
        assert_eq!(errors.len(), 2);
        assert!(
            errors
                .iter()
                .all(|error| error.message.contains("the public console"))
        );
        assert!(
            errors
                .iter()
                .all(|error| error.message.contains("HospitalPrivate"))
        );
    }

    #[test]
    fn permits_protected_computation_without_public_sink() {
        let source = "policy HospitalPrivate { owner: \"hospital-a\"; export: deny; } fn main() { let secret = private<HospitalPrivate>(100); let calculated = secret + 20; }";
        let program = parse_program(lex(source).unwrap()).unwrap();
        assert!(check(&program, &HashMap::new()).is_ok());
    }

    #[test]
    fn rejects_unknown_policy() {
        let program =
            parse_program(lex("fn main() { let secret = private<Missing>(1); }").unwrap()).unwrap();
        assert!(
            check(&program, &HashMap::new()).unwrap_err()[0]
                .message
                .contains("undefined policy")
        );
    }

    #[test]
    fn rejects_implicit_branch_and_loop_flows() {
        let source = "policy Secret { owner: \"org\"; export: deny; } fn main() { let secret = private<Secret>(2); if secret > 0 { println(\"branch\"); } let mut i = 0; while i < secret { println(\"loop\"); i = i + 1; } }";
        let program = parse_program(lex(source).unwrap()).unwrap();
        let errors = check(&program, &HashMap::new()).unwrap_err();
        assert_eq!(errors.len(), 2);
        assert!(
            errors
                .iter()
                .all(|error| error.message.contains("control-dependent"))
        );
    }

    #[test]
    fn propagates_labels_through_function_calls_and_returns() {
        let source = "policy Secret { owner: \"org\"; export: deny; } fn identity(x: int) -> int { return x; } fn expose(x: int) { println(x); } fn main() { let secret = private<Secret>(7); expose(secret); println(identity(secret)); }";
        let program = parse_program(lex(source).unwrap()).unwrap();
        let errors = check(&program, &HashMap::new()).unwrap_err();
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn rejects_protected_file_writes_and_error_paths() {
        let source = "policy Secret { owner: \"org\"; export: deny; } fn main() { let text = private<Secret>(\"data\"); write_file(\"out.txt\", text); let divisor = private<Secret>(1); let result = 10 / divisor; }";
        let program = parse_program(lex(source).unwrap()).unwrap();
        let errors = check(&program, &HashMap::new()).unwrap_err();
        assert_eq!(errors.len(), 2);
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("file output"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("runtime error channel"))
        );
    }
}
