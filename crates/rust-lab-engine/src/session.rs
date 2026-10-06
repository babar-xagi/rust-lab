use std::time::Instant;

use crate::{Backend, EngineError, Execution, Stage, Timings};

/// Explicit input modes avoid heuristic parsing of arbitrary Rust syntax.
#[derive(Debug, Clone)]
pub enum Cell {
    /// Statements/items in main's scope, retained after successful execution.
    Snippet(String),
    /// A Debug-printable expression, observed without adding it to history.
    Expression(String),
}

#[derive(Debug)]
pub struct Evaluation {
    pub execution: Execution,
    pub timings: Timings,
}

/// The backend owns compilation/runtime resources; the session owns semantics.
pub struct Session<B: Backend> {
    backend: B,
    history: Vec<String>,
}

impl<B: Backend> Session<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            history: Vec::new(),
        }
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }

    pub fn reset(&mut self) {
        self.history.clear();
    }

    /// Failed compilation or execution never commits source history.
    pub fn evaluate(&mut self, cell: Cell) -> Result<Evaluation, EngineError> {
        let total = Instant::now();
        let text = match &cell {
            Cell::Snippet(text) | Cell::Expression(text) => text,
        };
        if text.trim().is_empty() {
            return Err(EngineError::new(Stage::Input, "cell is empty"));
        }
        let start = Instant::now();
        let source = generate_source(&self.history, &cell);
        let source_generation = start.elapsed();
        let start = Instant::now();
        let prepared = self.backend.prepare(&source)?;
        let prepare = start.elapsed();
        let start = Instant::now();
        let program = self.backend.compile(prepared)?;
        let compile_link = start.elapsed();
        let start = Instant::now();
        let execution = self.backend.execute(program)?;
        let execute_process = start.elapsed();
        if execution.success
            && let Cell::Snippet(text) = cell
        {
            self.history.push(text);
        }
        Ok(Evaluation {
            execution,
            timings: Timings {
                source_generation,
                prepare,
                compile_link,
                execute_process,
                total: total.elapsed(),
            },
        })
    }
}

fn generate_source(history: &[String], cell: &Cell) -> String {
    let mut source = String::from("fn main() {\n");
    for snippet in history {
        source.push_str(snippet);
        source.push('\n');
    }
    match cell {
        Cell::Snippet(text) => source.push_str(text),
        Cell::Expression(text) => {
            source.push_str("::std::println!(\"{:?}\", {\n");
            source.push_str(text);
            source.push_str("\n});");
        }
    }
    source.push_str("\n}\n");
    source
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeBackend {
        sources: Vec<String>,
        fail_compile: bool,
        fail_execute: bool,
    }

    impl Backend for FakeBackend {
        type Prepared = String;
        type Program = String;

        fn prepare(&mut self, source: &str) -> Result<String, EngineError> {
            self.sources.push(source.to_owned());
            Ok(source.to_owned())
        }

        fn compile(&mut self, source: String) -> Result<String, EngineError> {
            if self.fail_compile {
                Err(EngineError::new(Stage::Compile, "bad source"))
            } else {
                Ok(source)
            }
        }

        fn execute(&mut self, _: String) -> Result<Execution, EngineError> {
            Ok(Execution {
                success: !self.fail_execute,
                exit_code: Some(i32::from(self.fail_execute)),
                stdout: String::new(),
                stderr: String::new(),
            })
        }
    }

    #[test]
    fn retains_snippets_but_not_observations_and_resets() {
        let mut session = Session::new(FakeBackend::default());
        session
            .evaluate(Cell::Snippet("let x = 7;".into()))
            .unwrap();
        let result = session.evaluate(Cell::Expression("x + 1".into())).unwrap();
        assert_eq!(session.history(), ["let x = 7;"]);
        assert!(session.backend.sources[1].contains("let x = 7;\n::std::println!"));
        assert!(result.timings.total >= result.timings.compile_link);
        session.reset();
        assert!(session.history().is_empty());
    }

    #[test]
    fn failed_cells_do_not_commit() {
        let mut session = Session::new(FakeBackend {
            fail_compile: true,
            ..Default::default()
        });
        assert_eq!(
            session
                .evaluate(Cell::Snippet("bad".into()))
                .unwrap_err()
                .stage,
            Stage::Compile
        );
        session.backend.fail_compile = false;
        session.backend.fail_execute = true;
        assert!(
            !session
                .evaluate(Cell::Snippet("panic!();".into()))
                .unwrap()
                .execution
                .success
        );
        assert!(session.history().is_empty());
    }

    #[test]
    fn empty_cell_never_reaches_backend() {
        let mut session = Session::new(FakeBackend::default());
        assert_eq!(
            session
                .evaluate(Cell::Expression(" \n".into()))
                .unwrap_err()
                .stage,
            Stage::Input
        );
        assert!(session.backend.sources.is_empty());
    }

    #[test]
    fn newline_keeps_trailing_comments_from_swallowing_generated_code() {
        let source = generate_source(
            &["let x = 7; // comment".into()],
            &Cell::Expression("x // value".into()),
        );
        assert!(source.contains("// comment\n::std::println!"));
        assert!(source.ends_with("// value\n});\n}\n"));
    }
}
