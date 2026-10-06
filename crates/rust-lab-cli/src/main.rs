mod options;

use std::fs;
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::process::{Command, ExitCode};
use std::time::Instant;

use options::{Action, BenchMode, Options};
use rust_lab_engine::timing::ms;
use rust_lab_engine::{CargoBackend, Cell, Evaluation, Session, Timings};

const HELP: &str = "rust-lab — Phase 1 Rust execution baseline

Usage:
  rust-lab                            Interactive session (also accepts piped lines)
  rust-lab --eval 'let x = 10; println!(\"{x}\");'
  rust-lab --expr '1 + 2'
  rust-lab --file cell.rs              Use '-' to read a whole snippet from stdin
  rust-lab bench [--mode changed|cached] [--iterations N] [--warmup N] [--expr CODE]

REPL: :expr CODE, :begin / :end for a multiline snippet, :reset, :help, :quit.
Snippets must include Rust semicolons where required. Expressions must implement Debug.
Session bindings are reconstructed by replay; previous side effects and output repeat.
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let options = Options::parse(std::env::args().skip(1).collect())?;
    if let Action::Help = options.action {
        print!("{HELP}");
        return Ok(());
    }
    if let Action::Bench {
        expression,
        mode,
        iterations,
        warmup,
    } = options.action
    {
        return benchmark(expression, mode, iterations, warmup);
    }
    let mut session = new_session()?;
    match options.action {
        Action::Repl => repl(&mut session),
        Action::Eval(cell) => {
            let result = session.evaluate(cell)?;
            display(&result)?;
            ensure_success(&result)
        }
        Action::File(path) => {
            let mut source = String::new();
            if path == "-" {
                io::stdin().read_to_string(&mut source)?;
            } else {
                source = fs::read_to_string(path)?;
            }
            let result = session.evaluate(Cell::Snippet(source))?;
            display(&result)?;
            ensure_success(&result)
        }
        Action::Help | Action::Bench { .. } => unreachable!(),
    }
}

fn new_session() -> Result<Session<CargoBackend>, Box<dyn std::error::Error>> {
    let start = Instant::now();
    let session = Session::new(CargoBackend::new()?);
    eprintln!("session_setup_ms={:.6}", ms(start.elapsed()));
    Ok(session)
}

fn display(result: &Evaluation) -> io::Result<()> {
    io::stdout().write_all(result.execution.stdout.as_bytes())?;
    io::stderr().write_all(result.execution.stderr.as_bytes())?;
    let t = result.timings;
    eprintln!(
        "source_generation_ms={:.6} prepare_ms={:.6} compile_link_ms={:.6} execute_process_ms={:.6} total_ms={:.6}",
        ms(t.source_generation),
        ms(t.prepare),
        ms(t.compile_link),
        ms(t.execute_process),
        ms(t.total)
    );
    Ok(())
}

fn ensure_success(result: &Evaluation) -> Result<(), Box<dyn std::error::Error>> {
    if result.execution.success {
        Ok(())
    } else {
        Err(format!("cell failed (exit code {:?})", result.execution.exit_code).into())
    }
}

fn repl(session: &mut Session<CargoBackend>) -> Result<(), Box<dyn std::error::Error>> {
    let interactive = io::stdin().is_terminal();
    if interactive {
        eprintln!("{HELP}");
    }
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut multiline: Option<String> = None;
    loop {
        if interactive {
            print!(
                "{}",
                if multiline.is_some() {
                    "... "
                } else {
                    "rust> "
                }
            );
            io::stdout().flush()?;
        }
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            if multiline.is_some() {
                return Err("unfinished multiline cell: expected :end".into());
            }
            break;
        }
        let trimmed = line.trim();
        let cell = if let Some(buffer) = &mut multiline {
            if trimmed == ":end" {
                Cell::Snippet(multiline.take().expect("active multiline buffer"))
            } else {
                buffer.push_str(&line);
                continue;
            }
        } else {
            match trimmed {
                "" => continue,
                ":quit" => break,
                ":help" => {
                    eprintln!("{HELP}");
                    continue;
                }
                ":reset" => {
                    session.reset();
                    eprintln!("session history cleared");
                    continue;
                }
                ":begin" => {
                    multiline = Some(String::new());
                    continue;
                }
                _ => {
                    if let Some(expression) = trimmed.strip_prefix(":expr ") {
                        Cell::Expression(expression.to_owned())
                    } else if trimmed.starts_with(':') {
                        eprintln!("unknown command; use :help");
                        continue;
                    } else {
                        Cell::Snippet(line)
                    }
                }
            }
        };
        match session.evaluate(cell) {
            Ok(result) => {
                display(&result)?;
                if !result.execution.success {
                    eprintln!("cell failed; source history unchanged");
                }
            }
            Err(err) => eprintln!("{err}"),
        }
    }
    Ok(())
}

fn benchmark(
    expression: String,
    mode: BenchMode,
    iterations: usize,
    warmup: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    for tool in ["rustc", "cargo"] {
        let output = Command::new(tool).arg("--version").output()?;
        if !output.status.success() {
            return Err(format!("could not query {tool} version").into());
        }
        eprint!("{}", String::from_utf8_lossy(&output.stdout));
    }
    eprintln!(
        "backend=cargo-executable profile=dev os={} arch={} mode={mode:?} iterations={iterations} warmup={warmup}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let mut session = new_session()?;
    let cell = |index: usize| {
        let comment = match mode {
            BenchMode::Changed => format!("// benchmark source revision {index}\n"),
            BenchMode::Cached => String::new(),
        };
        // Black-box the value so basic computations remain observable to rustc.
        Cell::Expression(format!(
            "{comment}::std::hint::black_box({{\n{expression}\n}})"
        ))
    };
    for index in 0..warmup {
        ensure_success(&session.evaluate(cell(index))?)?;
    }
    let mut samples = Vec::with_capacity(iterations);
    println!("condition,{}", Timings::csv_header());
    for sample in 0..iterations {
        let result = session.evaluate(cell(warmup + sample))?;
        ensure_success(&result)?;
        let condition = if warmup == 0 && sample == 0 {
            "first-build"
        } else {
            match mode {
                BenchMode::Changed => "source-change",
                BenchMode::Cached => "cache-hit",
            }
        };
        println!("{condition},{}", result.timings.csv_row(sample + 1));
        samples.push(ms(result.timings.total));
    }
    samples.sort_by(f64::total_cmp);
    let percentile = |p: usize| samples[(samples.len() * p).div_ceil(100) - 1];
    eprintln!(
        "total_ms min={:.6} p50={:.6} p95={:.6} max={:.6} (nearest-rank percentiles)",
        samples[0],
        percentile(50),
        percentile(95),
        samples[samples.len() - 1]
    );
    Ok(())
}
