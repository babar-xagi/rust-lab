use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rust-lab"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn expression_output_timings_and_failure_exit_status() {
    let output = run(&["--expr", "6 * 7"], "");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("session_setup_ms="));
    assert!(stderr.contains("compile_link_ms="));
    assert!(stderr.contains("execute_process_ms="));
    let compile_error = run(&["--expr", "missing_name"], "");
    assert!(!compile_error.status.success());
    assert!(String::from_utf8_lossy(&compile_error.stderr).contains("missing_name"));
    let panic = run(&["--eval", "panic!(\"cli panic\");"], "");
    assert!(!panic.status.success());
    assert!(String::from_utf8_lossy(&panic.stderr).contains("cli panic"));
}

#[test]
fn piped_session_multiline_recovery_and_reset() {
    let output = run(
        &[],
        ":begin\nfn twice(x: i32) -> i32 {\nx * 2\n}\nlet x = twice(5);\n:end\n:expr missing_name\n:expr x + 20\n:reset\n:expr x\n:expr 1\n:quit\n",
    );
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "30\n1\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing_name"));
    assert!(!run(&[], ":begin\nlet x = 1;\n").status.success());
}

#[test]
fn file_and_whole_stdin_snippets() {
    let path = format!("{}/../../examples/cell.rs", env!("CARGO_MANIFEST_DIR"));
    let output = run(&["--file", &path], "");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "total = 10");
    let stdin = run(&["--file", "-"], "let x = 2;\nprintln!(\"{}\", x * 2);\n");
    assert!(stdin.status.success());
    assert_eq!(String::from_utf8_lossy(&stdin.stdout).trim(), "4");
}

#[test]
fn benchmark_csv_distinguishes_build_conditions_and_discards_cell_output() {
    let output = run(
        &[
            "bench",
            "--iterations",
            "3",
            "--warmup",
            "0",
            "--expr",
            "{ println!(\"cell output\"); 3 }",
        ],
        "",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let csv = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = csv.lines().collect();
    assert_eq!(lines.len(), 4);
    assert_eq!(
        lines[0],
        "condition,sample,source_generation_ms,prepare_ms,compile_link_ms,execute_process_ms,total_ms"
    );
    for (index, line) in lines[1..].iter().enumerate() {
        let columns: Vec<_> = line.split(',').collect();
        assert_eq!(columns.len(), 7);
        assert_eq!(
            columns[0],
            if index == 0 {
                "first-build"
            } else {
                "source-change"
            }
        );
        assert_eq!(columns[1].parse::<usize>().unwrap(), index + 1);
        let stages: Vec<f64> = columns[2..].iter().map(|s| s.parse().unwrap()).collect();
        assert!(
            stages
                .iter()
                .all(|value| value.is_finite() && *value >= 0.0)
        );
        assert!(stages[4] + 0.000005 >= stages[..4].iter().sum::<f64>());
    }
    let metadata = String::from_utf8_lossy(&output.stderr);
    assert!(metadata.contains("rustc "));
    assert!(metadata.contains("p95="));
    let cached = run(
        &[
            "bench",
            "--mode",
            "cached",
            "--iterations",
            "2",
            "--warmup",
            "1",
        ],
        "",
    );
    assert!(cached.status.success());
    assert!(
        String::from_utf8_lossy(&cached.stdout)
            .lines()
            .skip(1)
            .all(|line| line.starts_with("cache-hit,"))
    );
    let failed = run(
        &[
            "bench",
            "--iterations",
            "1",
            "--warmup",
            "0",
            "--expr",
            "panic!(\"benchmark panic\")",
        ],
        "",
    );
    assert!(!failed.status.success());
    assert_eq!(String::from_utf8_lossy(&failed.stdout).lines().count(), 1);
}
