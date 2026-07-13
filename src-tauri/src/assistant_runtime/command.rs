use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use tokio::process::Command;

/// Injectable program, arguments, and environment for one sidecar process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssistantSidecarCommand {
    program: OsString,
    args: Vec<OsString>,
    env: BTreeMap<OsString, OsString>,
    current_dir: Option<PathBuf>,
}

impl AssistantSidecarCommand {
    /// Creates a sidecar command using the inherited process environment.
    pub fn new(program: impl Into<OsString>) -> Self {
        Self { program: program.into(), args: Vec::new(), env: BTreeMap::new(), current_dir: None }
    }

    /// Appends command-line arguments.
    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args.extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));
        self
    }

    /// Sets one environment variable for the child process.
    #[must_use]
    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Sets the child working directory.
    #[must_use]
    pub fn current_dir(mut self, directory: impl AsRef<Path>) -> Self {
        self.current_dir = Some(directory.as_ref().to_owned());
        self
    }

    pub(super) fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args).envs(&self.env);
        if let Some(current_dir) = &self.current_dir {
            command.current_dir(current_dir);
        }
        command
    }
}
