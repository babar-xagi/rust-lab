use rust_lab_engine::{CargoBackend, Cell, Session, Stage};

#[test]
fn real_cargo_session_replays_bindings_and_recovers_from_errors() {
    let backend = CargoBackend::new().unwrap();
    let directory = backend.directory().to_owned();
    {
        let mut session = Session::new(backend);
        assert!(
            session
                .evaluate(Cell::Snippet("let mut x = 10;".into()))
                .unwrap()
                .execution
                .success
        );
        session.evaluate(Cell::Snippet("x += 2;".into())).unwrap();
        let result = session.evaluate(Cell::Expression("x + 20".into())).unwrap();
        assert_eq!(result.execution.stdout.trim(), "32");
        assert!(result.timings.total >= result.timings.execute_process);

        let error = session
            .evaluate(Cell::Snippet("let broken = missing_name;".into()))
            .unwrap_err();
        assert_eq!(error.stage, Stage::Compile);
        assert!(error.message.contains("missing_name"));
        assert_eq!(session.history().len(), 2);

        let panic = session
            .evaluate(Cell::Snippet(
                "eprintln!(\"before panic\"); panic!(\"test panic\");".into(),
            ))
            .unwrap();
        assert!(!panic.execution.success);
        assert!(panic.execution.stderr.contains("test panic"));
        assert_eq!(session.history().len(), 2);
        let recovered = session.evaluate(Cell::Expression("x".into())).unwrap();
        assert_eq!(recovered.execution.stdout.trim(), "12");
        session
            .evaluate(Cell::Expression("{ x += 100; x }".into()))
            .unwrap();
        assert_eq!(
            session
                .evaluate(Cell::Expression("x".into()))
                .unwrap()
                .execution
                .stdout
                .trim(),
            "12",
            "expression mutations are observations, not committed source"
        );
        session.reset();
        let reset = session.evaluate(Cell::Expression("x".into())).unwrap_err();
        assert_eq!(reset.stage, Stage::Compile);
    }
    assert!(
        !directory.exists(),
        "backend should clean up its own temporary directory"
    );
}

#[test]
fn sessions_are_isolated_and_multiline_code_works() {
    let mut first = Session::new(CargoBackend::new().unwrap());
    let mut second = Session::new(CargoBackend::new().unwrap());
    first
        .evaluate(Cell::Snippet(
            "fn twice(x: i32) -> i32 {\n x * 2\n}\nlet value = twice(9);".into(),
        ))
        .unwrap();
    assert_eq!(
        first
            .evaluate(Cell::Expression("value".into()))
            .unwrap()
            .execution
            .stdout
            .trim(),
        "18"
    );
    assert_eq!(
        second
            .evaluate(Cell::Expression("value".into()))
            .unwrap_err()
            .stage,
        Stage::Compile
    );
    assert_eq!(
        second
            .evaluate(Cell::Expression("\"hello\"".into()))
            .unwrap()
            .execution
            .stdout
            .trim(),
        "\"hello\""
    );
}

#[test]
fn replay_repeats_prior_output_and_preserves_shadowing() {
    let mut session = Session::new(CargoBackend::new().unwrap());
    session
        .evaluate(Cell::Snippet("let x = 1; println!(\"prior={x}\");".into()))
        .unwrap();
    session
        .evaluate(Cell::Snippet("let x = 9;".into()))
        .unwrap();
    let result = session.evaluate(Cell::Expression("x".into())).unwrap();
    assert_eq!(result.execution.stdout, "prior=1\n9\n");
}
