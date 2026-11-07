use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

#[test]
fn eval_command_prints_result() {
    let output = Command::new(env!("CARGO_BIN_EXE_veyra"))
        .args(["eval", "2 + 3 * (4 - 1)"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "11\n");
    assert_eq!(output.stderr, Vec::<u8>::new());
}

#[test]
fn runtime_error_has_nonzero_status_and_diagnostic() {
    let output = Command::new(env!("CARGO_BIN_EXE_veyra"))
        .args(["eval", "1 / 0"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("division by zero"));
    assert!(stderr.contains("<argument>:1:5"));
}

fn fixture() -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "veyra-tests-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("main.vy");
    fs::write(&path, "fn fact(n: int) -> int { if n <= 1 { return 1; } return n * fact(n - 1); } fn main() { let values = [1, 2, 3]; for i in 0..len(values) { println(i); } println(fact(5)); }").unwrap();
    path
}

#[test]
fn interpreter_and_vm_agree() {
    let path = fixture();
    let vm = Command::new(env!("CARGO_BIN_EXE_veyra"))
        .args(["run", path.to_str().unwrap()])
        .output()
        .unwrap();
    let interpreter = Command::new(env!("CARGO_BIN_EXE_veyra"))
        .args(["run", "--interpreter", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        vm.status.success(),
        "{}",
        String::from_utf8_lossy(&vm.stderr)
    );
    assert!(
        interpreter.status.success(),
        "{}",
        String::from_utf8_lossy(&interpreter.stderr)
    );
    assert_eq!(vm.stdout, interpreter.stdout);
    assert_eq!(String::from_utf8(vm.stdout).unwrap(), "0\n1\n2\n120\n");
}

#[test]
fn built_bytecode_runs_without_source() {
    let source = fixture();
    let output = source.with_extension("vyb");
    let build = Command::new(env!("CARGO_BIN_EXE_veyra"))
        .args([
            "build",
            source.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    fs::remove_file(&source).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_veyra"))
        .args(["run", output.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8(run.stdout).unwrap(), "0\n1\n2\n120\n");
}

fn repository_fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

#[test]
fn required_negative_compilation_fixtures_are_enforced() {
    let cases = [
        (
            "tests/fixtures/negative/type-mismatch.vy",
            "expected int, found string",
        ),
        (
            "tests/fixtures/negative/implicit-flow.vy",
            "control-dependent",
        ),
        (
            "tests/fixtures/negative/protected-file-write.vy",
            "file output",
        ),
        (
            "tests/fixtures/negative/unauthorized-release.vy",
            "not callable",
        ),
        (
            "tests/fixtures/negative/cross-module-flow.vy",
            "public console",
        ),
    ];
    for (fixture, expected) in cases {
        let path = repository_fixture(fixture);
        let output = Command::new(env!("CARGO_BIN_EXE_veyra"))
            .args(["check", path.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "fixture unexpectedly passed: {fixture}"
        );
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains(expected),
            "{fixture}: expected `{expected}` in {stderr}"
        );
    }
}

#[test]
fn positive_policy_fixture_and_multi_file_example_work() {
    let safe = Command::new(env!("CARGO_BIN_EXE_veyra"))
        .args([
            "check",
            repository_fixture("tests/fixtures/positive/protected-computation.vy")
                .to_str()
                .unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        safe.status.success(),
        "{}",
        String::from_utf8_lossy(&safe.stderr)
    );

    let modules = Command::new(env!("CARGO_BIN_EXE_veyra"))
        .args([
            "run",
            repository_fixture("examples/modules/main.vy")
                .to_str()
                .unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        modules.status.success(),
        "{}",
        String::from_utf8_lossy(&modules.stderr)
    );
    assert_eq!(String::from_utf8(modules.stdout).unwrap(), "49\n");
}
