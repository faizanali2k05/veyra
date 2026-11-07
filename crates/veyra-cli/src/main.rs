use std::{
    env, fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};
use veyra_bytecode::{compile, decode, disassemble, encode};
use veyra_cli::{default_build_path, eval, load_project, render_error};
use veyra_interpreter::Interpreter;
use veyra_sema::check;
use veyra_source::Source;
use veyra_vm::Vm;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}
fn run() -> Result<(), String> {
    let args: Vec<String> = env::args_os()
        .skip(1)
        .map(|v| {
            v.into_string()
                .map_err(|_| "arguments must be valid UTF-8".to_owned())
        })
        .collect::<Result<_, _>>()?;
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    match command {
        "eval" => {
            if args.len() != 2 {
                return Err("Usage: veyra eval <expression>".into());
            }
            let source = Source::new("<argument>", args[1].clone());
            match eval(&source) {
                Ok(v) => println!("{v}"),
                Err(e) => return Err(render_error(&source, &e)),
            }
            Ok(())
        }
        "run" => run_file(&args[1..]),
        "check" => check_file(one_path(&args, "check")?),
        "build" => build_file(&args[1..]),
        "disasm" => disasm_file(one_path(&args, "disasm")?),
        "repl" => {
            if args.len() != 1 {
                return Err("Usage: veyra repl".into());
            }
            repl()
        }
        "fmt" => format_files(&args[1..]),
        "--help" | "-h" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        "--version" | "-V" => {
            println!("veyra {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => Err(usage()),
    }
}
fn usage() -> String {
    "Veyra 0.1\n\nUsage:\n  veyra eval <expression>\n  veyra run [--interpreter] <file.vy|file.vyb>\n  veyra check <file.vy>\n  veyra build <file.vy> [-o output.vyb]\n  veyra repl\n  veyra fmt <file.vy>...\n  veyra disasm <file.vy|file.vyb>".into()
}
fn one_path<'a>(args: &'a [String], command: &str) -> Result<&'a Path, String> {
    if args.len() != 2 {
        return Err(format!("Usage: veyra {command} <file>"));
    }
    Ok(Path::new(&args[1]))
}
fn checked_project(path: &Path) -> Result<veyra_cli::Project, String> {
    let p = load_project(path)?;
    if let Err(errors) = check(&p.root, &p.modules) {
        let rendered = errors
            .iter()
            .map(|e| p.root_source.render(&e.message, e.span))
            .collect::<Vec<_>>()
            .join("\n\n");
        return Err(rendered);
    }
    Ok(p)
}
fn run_file(args: &[String]) -> Result<(), String> {
    let (interpreter, path) = match args {
        [path] => (false, Path::new(path)),
        [flag, path] if flag == "--interpreter" => (true, Path::new(path)),
        _ => return Err("Usage: veyra run [--interpreter] <file.vy|file.vyb>".into()),
    };
    if path.extension().is_some_and(|e| e == "vyb") {
        if interpreter {
            return Err("--interpreter only accepts Veyra source".into());
        }
        let bytes = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        return run_vm(decode(&bytes)?, None);
    }
    let p = checked_project(path)?;
    if interpreter {
        let mut runtime = Interpreter::new();
        runtime
            .run(&p.root, &p.modules)
            .map_err(|e| p.root_source.render(&e.message, e.span))?;
        for line in runtime.output() {
            println!("{line}");
        }
    } else {
        run_vm(
            compile(&p.root, &p.modules).map_err(|e| p.root_source.render(&e.message, e.span))?,
            Some(&p.root_source),
        )?;
    }
    Ok(())
}
fn run_vm(bytecode: veyra_bytecode::Bytecode, source: Option<&Source>) -> Result<(), String> {
    let mut vm = Vm::new(bytecode).map_err(|e| e.to_string())?;
    vm.run().map_err(|error| {
        source.map_or_else(
            || error.to_string(),
            |source| source.render(&error.message, error.span),
        )
    })?;
    for line in vm.output() {
        println!("{line}");
    }
    Ok(())
}
fn check_file(path: &Path) -> Result<(), String> {
    checked_project(path)?;
    println!("{}: OK", path.display());
    Ok(())
}
fn build_file(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("Usage: veyra build <file.vy> [-o output.vyb]".into());
    }
    let source = Path::new(&args[0]);
    let output = match args {
        [_] => default_build_path(source),
        [_, flag, path] if flag == "-o" => PathBuf::from(path),
        _ => return Err("Usage: veyra build <file.vy> [-o output.vyb]".into()),
    };
    let p = checked_project(source)?;
    let bytecode =
        compile(&p.root, &p.modules).map_err(|e| p.root_source.render(&e.message, e.span))?;
    fs::write(&output, encode(&bytecode)?)
        .map_err(|e| format!("cannot write {}: {e}", output.display()))?;
    println!("built {}", output.display());
    Ok(())
}
fn disasm_file(path: &Path) -> Result<(), String> {
    let bytecode = if path.extension().is_some_and(|e| e == "vyb") {
        decode(&fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?)?
    } else {
        let p = checked_project(path)?;
        compile(&p.root, &p.modules).map_err(|e| e.to_string())?
    };
    print!("{}", disassemble(&bytecode));
    Ok(())
}
fn repl() -> Result<(), String> {
    println!("Veyra REPL (integer expressions; Ctrl-D to exit)");
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let source = Source::new("<repl>", line);
        match eval(&source) {
            Ok(v) => println!("{v}"),
            Err(e) => eprintln!("{}", render_error(&source, &e)),
        }
        print!("> ");
        stdout.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn format_files(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("Usage: veyra fmt <file.vy>...".into());
    }
    for path in paths {
        let path = Path::new(path);
        let input =
            fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let source = Source::new(path.display().to_string(), input.clone());
        veyra_parser::parse_program(
            veyra_lexer::lex(&input).map_err(|e| source.render(&e.message, e.span))?,
        )
        .map_err(|e| source.render(&e.message, e.span))?;
        let formatted = input
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        fs::write(path, formatted).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    Ok(())
}
