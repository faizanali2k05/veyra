# Veyra Language Specification (draft 0.1)

This document fixes the intended Veyra 1.0 semantics. A feature is not considered
implemented merely because it appears here; the implementation status is tracked
in `ROADMAP.md`.

## Source and lexical rules

Source files are UTF-8 and use the `.vy` extension. Identifiers begin with an
underscore or Unicode alphabetic character and continue with underscores or
Unicode alphanumeric characters. Keywords are reserved. `//` introduces a line
comment. Whitespace has no semantic meaning. All diagnostics use one-based line
and column numbers and byte-offset source spans.

## Types and bindings

Primitive types are `int` (signed 64-bit), `float` (IEEE-754 binary64), `bool`,
UTF-8 `string`, and `unit`. Arrays are homogeneous and dynamically sized.
Bindings are immutable unless declared `let mut`. Local binding types may be
inferred from initializers; function parameter and return types are explicit
(an omitted return annotation means `unit`). There are no implicit numeric
conversions and conditions must be `bool`.

## Expressions and evaluation

Operands and function arguments evaluate left-to-right. `&&` and `||` short
circuit. Operator precedence, highest first, is:

1. postfix call, field access, and indexing
2. unary `-` and `!`
3. `*`, `/`, `%`
4. `+`, `-`
5. `<`, `<=`, `>`, `>=`
6. `==`, `!=`
7. `&&`
8. `||`

Binary operators of equal precedence associate left. Integer arithmetic is
checked and overflow is a runtime error. Integer division truncates toward zero.
The remainder has the dividend's sign and satisfies `a == (a / b) * b + a % b`.
Division or remainder by zero is a runtime error. Floating-point behavior follows
IEEE-754 binary64. Strings compare by Unicode scalar-value sequence; equality is
content equality. Array indices are zero-based and bounds checked.

## Declarations and control flow

Lexical block scope applies to `let`, `fn`, and parameters. The declarations are
`let`, `fn`, `struct`, and `import`; control flow consists of `if`/`else`, `while`,
`for`, and `return`. `for i in a..b` uses a half-open range. Every source program
executes through `fn main()` except a CLI expression evaluated with `eval`.

## Grammar

```ebnf
program       = { declaration } EOF ;
declaration   = let_decl | fn_decl | struct_decl | import_decl | policy_decl ;
let_decl      = "let" [ "mut" ] IDENT [ ":" type ] "=" expression ";" ;
policy_decl   = "policy" IDENT "{" "owner" ":" STRING ";"
                "export" ":" "deny" ";" "}" ;
fn_decl       = [ "pub" ] "fn" IDENT "(" [ parameters ] ")"
                [ "->" type ] block ;
parameters    = parameter { "," parameter } [ "," ] ;
parameter     = IDENT ":" type ;
struct_decl   = "struct" IDENT "{" { IDENT ":" type "," } "}" ;
import_decl   = "import" IDENT ";" ;
block         = "{" { statement } "}" ;
statement     = declaration | expression ";" | assignment ";" | if_stmt
              | while_stmt | for_stmt | return_stmt ;
expression    = logical_or ;
logical_or    = logical_and { "||" logical_and } ;
logical_and   = equality { "&&" equality } ;
equality      = comparison { ( "==" | "!=" ) comparison } ;
comparison    = term { ( "<" | "<=" | ">" | ">=" ) term } ;
term          = factor { ( "+" | "-" ) factor } ;
factor        = unary { ( "*" | "/" | "%" ) unary } ;
unary         = ( "-" | "!" ) unary | postfix ;
postfix       = primary { call | index | field } ;
primary       = literal | IDENT | array | struct_value | private_value
              | "(" expression ")" ;
private_value = "private" "<" IDENT ">" "(" expression ")" ;
type          = "int" | "float" | "bool" | "string" | "unit" | IDENT
              | "[" type "]" ;
```

This grammar is implemented by the 1.0 parser. Parsing stops at the first syntax
error; no invalid program is silently recovered or executed.

## Frozen 1.0 decisions

- `import name;` loads sibling `name.vy`; repeated/cyclic discovery is deduplicated.
- Imported functions require `pub`; module globals are not supported.
- Arrays and structs are shared managed values; equality is structural.
- Managed aggregates use safe reference counting. Cycles are not reclaimed.
- Strings cannot be indexed in 1.0; `len(string)` counts Unicode characters.
- `.vyb` files use magic `VYBC` and bytecode version 1. Loaders reject other versions.
- The built-ins are `println`, `len`, `input`, `read_file`, and `write_file`.

Generics, inheritance, macros, async/await, and native code generation are out of
scope for version 1.

## Policy-aware extension status

Named confidentiality policies and `private<Policy>(value)` are implemented.
Labels propagate through expressions, calls, returns, sibling modules, and the
program-counter of `if`, `while`, and `for`. The checker rejects protected direct
or control-dependent flows to `println` and `write_file`, plus conservative
division/remainder and indexing error channels. Labels are compile-time metadata
and are erased only after successful checking.

Capability-controlled release and further trusted sinks are specified in
`POLICY_SEMANTICS.md` and staged in `ROADMAP.md`. No release or downgrade
operation currently exists.

