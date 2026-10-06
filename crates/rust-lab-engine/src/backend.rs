use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{EngineError, Stage};

/// Future backends may prepare compiler IR and return in-memory JIT handles.
/// Neither session nor CLI needs to know the representation of these handles.
pub trait Backend {
    type Prepared;
    type Program;

    fn prepare(&mut self, source: &str) -> Result<Self::Prepared, EngineError>;
    fn compile(&mut self, prepared: Self::Prepared) -> Result<Self::Program, EngineError>;
    fn execute(&mut self, program: Self::Program) -> Result<Execution, EngineError>;
}

#[derive(Debug)]
pub struct Execution {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// One independent temporary Cargo project and incremental cache per session.
pub struct CargoBackend {
    directory: PathBuf,
}

impl CargoBackend {
    pub fn new() -> Result<Self, EngineError> {
        let directory = allocate_directory()?;
        let backend = Self { directory };
        fs::create_dir(backend.directory.join("src"))
            .map_err(|err| EngineError::new(Stage::Prepare, err.to_string()))?;
        fs::write(backend.directory.join("Cargo.toml"),
            "[package]\nname = \"rust_lab_cell\"\nversion = \"0.0.0\"\nedition = \"2024\"\nrust-version = \"1.99\"\n\n[workspace]\n")
            .map_err(|err| EngineError::new(Stage::Prepare, err.to_string()))?;
        Ok(backend)
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

impl Backend for CargoBackend {
    type Prepared = ();
    type Program = PathBuf;

    fn prepare(&mut self, source: &str) -> Result<(), EngineError> {
        let path = self.directory.join("src/main.rs");
        // Identical-source evaluations deliberately preserve Cargo's cache.
        if fs::read_to_string(&path).ok().as_deref() != Some(source) {
            fs::write(path, source)
                .map_err(|err| EngineError::new(Stage::Prepare, err.to_string()))?;
        }
        Ok(())
    }

    fn compile(&mut self, _: ()) -> Result<PathBuf, EngineError> {
        let output = Command::new("cargo")
            .args(["build", "--offline", "--quiet"])
            .arg("--manifest-path")
            .arg(self.directory.join("Cargo.toml"))
            .arg("--target-dir")
            .arg(self.directory.join("target"))
            .current_dir(&self.directory)
            .stdin(Stdio::null())
            .output()
            .map_err(|err| {
                EngineError::new(Stage::Compile, format!("cannot start cargo: {err}"))
            })?;
        if !output.status.success() {
            return Err(EngineError::new(
                Stage::Compile,
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }
        Ok(self
            .directory
            .join("target/debug")
            .join(format!("rust_lab_cell{}", std::env::consts::EXE_SUFFIX)))
    }

    fn execute(&mut self, program: PathBuf) -> Result<Execution, EngineError> {
        let output = Command::new(program)
            .current_dir(&self.directory)
            .stdin(Stdio::null())
            .output()
            .map_err(|err| EngineError::new(Stage::Execute, format!("cannot start cell: {err}")))?;
        Ok(Execution {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

impl Drop for CargoBackend {
    fn drop(&mut self) {
        // Only the unique directory successfully created by this instance.
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn allocate_directory() -> Result<PathBuf, EngineError> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for _ in 0..100 {
        let directory = std::env::temp_dir().join(format!(
            "rust-lab-{}-{epoch}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&directory) {
            Ok(()) => return Ok(directory),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(EngineError::new(Stage::Prepare, err.to_string())),
        }
    }
    Err(EngineError::new(
        Stage::Prepare,
        "could not allocate a unique session directory",
    ))
}
