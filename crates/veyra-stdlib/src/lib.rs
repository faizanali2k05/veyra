//! Names and documentation for the Veyra 1.0 host-provided standard library.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Builtin {
    pub name: &'static str,
    pub signature: &'static str,
    pub description: &'static str,
}

pub const BUILTINS: &[Builtin] = &[
    Builtin {
        name: "println",
        signature: "fn(any) -> unit",
        description: "Print a value followed by a newline.",
    },
    Builtin {
        name: "len",
        signature: "fn([T] | string) -> int",
        description: "Return an array length or Unicode character count.",
    },
    Builtin {
        name: "input",
        signature: "fn(string) -> string",
        description: "Print a prompt and read one line from standard input.",
    },
    Builtin {
        name: "read_file",
        signature: "fn(string) -> string",
        description: "Read a UTF-8 file or raise a runtime error.",
    },
    Builtin {
        name: "write_file",
        signature: "fn(string, string) -> unit",
        description: "Write UTF-8 contents to a file.",
    },
];
