use rust_lab_engine::Cell;

#[derive(Debug)]
pub enum Action {
    Help,
    Repl,
    Eval(Cell),
    File(String),
    Bench {
        expression: String,
        mode: BenchMode,
        iterations: usize,
        warmup: usize,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum BenchMode {
    Changed,
    Cached,
}

pub struct Options {
    pub action: Action,
}

impl Options {
    pub fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut args = args.into_iter();
        let action = match args.next().as_deref() {
            None => Action::Repl,
            Some("--help" | "-h") => Action::Help,
            Some("--eval") => Action::Eval(Cell::Snippet(value(&mut args, "--eval")?)),
            Some("--expr") => Action::Eval(Cell::Expression(value(&mut args, "--expr")?)),
            Some("--file") => Action::File(value(&mut args, "--file")?),
            Some("bench") => {
                let mut expression = "1u64 + 2".to_owned();
                let mut mode = BenchMode::Changed;
                let mut iterations = 10;
                let mut warmup = 1;
                while let Some(flag) = args.next() {
                    match flag.as_str() {
                        "--expr" => expression = value(&mut args, &flag)?,
                        "--iterations" => iterations = number(&mut args, &flag)?,
                        "--warmup" => warmup = number(&mut args, &flag)?,
                        "--mode" => {
                            mode = match value(&mut args, &flag)?.as_str() {
                                "changed" => BenchMode::Changed,
                                "cached" => BenchMode::Cached,
                                other => return Err(format!("unknown benchmark mode: {other}")),
                            }
                        }
                        _ => return Err(format!("unknown benchmark option: {flag}")),
                    }
                }
                if iterations == 0 {
                    return Err("--iterations must be greater than zero".into());
                }
                if expression.trim().is_empty() {
                    return Err("benchmark expression is empty".into());
                }
                if warmup.checked_add(iterations).is_none() {
                    return Err("sample count overflow".into());
                }
                Action::Bench {
                    expression,
                    mode,
                    iterations,
                    warmup,
                }
            }
            Some(other) => return Err(format!("unknown argument: {other}; use --help")),
        };
        if let Some(extra) = args.next() {
            return Err(format!("unexpected argument: {extra}"));
        }
        Ok(Self { action })
    }
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn number(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<usize, String> {
    value(args, flag)?
        .parse()
        .map_err(|_| format!("{flag} expects a nonnegative integer"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::parse(args.iter().map(|s| (*s).to_owned()).collect())
    }

    #[test]
    fn rejects_invalid_cli_arguments() {
        for args in [
            vec!["--expr"],
            vec!["--eval", "x", "extra"],
            vec!["bench", "--iterations", "0"],
            vec!["bench", "--warmup", "-1"],
            vec!["bench", "--mode", "unknown"],
            vec!["bench", "--expr", " "],
            vec!["bench", "--unknown"],
            vec!["--unknown"],
        ] {
            assert!(parse(&args).is_err(), "accepted {args:?}");
        }
    }

    #[test]
    fn defaults_and_explicit_modes() {
        assert!(matches!(parse(&[]).unwrap().action, Action::Repl));
        assert!(matches!(
            parse(&["--expr", "1+2"]).unwrap().action,
            Action::Eval(Cell::Expression(_))
        ));
        assert!(matches!(
            parse(&[
                "bench",
                "--mode",
                "cached",
                "--iterations",
                "2",
                "--warmup",
                "0"
            ])
            .unwrap()
            .action,
            Action::Bench {
                mode: BenchMode::Cached,
                iterations: 2,
                warmup: 0,
                ..
            }
        ));
    }
}
