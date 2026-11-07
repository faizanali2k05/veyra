# Using Veyra

## Variables and values

Bindings are immutable unless `mut` is written. Types can be inferred locally.

```veyra
let name = "Faizan";
let age: int = 22;
let mut score = 0;
score = score + 10;
```

The primitive types are `int`, `float`, `bool`, `string`, and `unit`. Integers are
checked signed 64-bit values; floats are binary64. There are no implicit numeric
conversions.

## Functions and control flow

```veyra
fn factorial(n: int) -> int {
    if n <= 1 {
        return 1;
    }
    return n * factorial(n - 1);
}

fn main() {
    let mut count = 0;
    while count < 3 {
        println(count);
        count = count + 1;
    }
    for i in 0..5 {
        println(i);
    }
}
```

Every executable source file needs `fn main()`. Conditions must be `bool`;
Veyra has no truthiness. Ranges are half-open.

## Arrays and structs

```veyra
struct Student {
    name: string,
    age: int,
}

fn main() {
    let scores = [90, 85, 95];
    let student = Student { name: "Ali", age: 22, };
    println(student.name);
    println(scores[0]);
    println(len(scores));
}
```

Arrays are homogeneous and bounds checked. Array type annotations use `[int]` or
`[string]`. Empty arrays are rejected because their element type cannot be inferred.

## Modules

For `import math;`, Veyra loads sibling file `math.vy`. Only `pub` functions are
callable through a module.

```veyra
// math.vy
pub fn square(x: int) -> int {
    return x * x;
}
```

```veyra
// main.vy
import math;
fn main() {
    println(math.square(7));
}
```

## Built-ins

- `println(value) -> unit` prints a value and newline.
- `len(array_or_string) -> int` counts elements or Unicode characters.
- `input(prompt: string) -> string` reads one input line.
- `read_file(path: string) -> string` reads a UTF-8 file.
- `write_file(path: string, contents: string) -> unit` writes a file.

## Building bytecode

```sh
veyra build main.vy
veyra run main.vyb
veyra disasm main.vyb
```

The `.vyb` file contains compiled instructions and imported code, so it runs
without the source files. It is portable Veyra bytecode, not native machine code.

