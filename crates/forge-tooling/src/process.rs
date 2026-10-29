//! Running external programs with argument arrays (never through a shell,
//! SPEC §11.1), behind a trait so tools can be tested with scripted output.

use std::io;
use std::path::Path;
use std::process::Command;

/// Captured result of a finished process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    /// Exit code, `None` if killed by a signal.
    pub status: Option<i32>,
    /// Standard output, lossily decoded.
    pub stdout: String,
    /// Standard error, lossily decoded.
    pub stderr: String,
}

/// Something that can run a program to completion.
pub trait CommandRunner {
    /// Runs `program` with `args` in `cwd` and waits for it.
    ///
    /// # Errors
    ///
    /// Returns the I/O error when the program cannot be started.
    fn run(&self, program: &str, args: &[&str], cwd: &Path) -> io::Result<CommandOutput>;
}

/// The real runner, backed by [`std::process::Command`].
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&str], cwd: &Path) -> io::Result<CommandOutput> {
        let output = Command::new(program).args(args).current_dir(cwd).output()?;
        Ok(CommandOutput {
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}
