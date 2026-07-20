//! Isolated Assistant protocol process command selected by the composition root.

use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

use tokio::process::Command;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssistantSidecarCommand {
    program: OsString,
    args: Vec<OsString>,
    env: BTreeMap<OsString, OsString>,
    current_dir: Option<PathBuf>,
}

impl AssistantSidecarCommand {
    pub fn new(program: impl Into<OsString>) -> Self {
        Self { program: program.into(), args: Vec::new(), env: BTreeMap::new(), current_dir: None }
    }

    pub fn development(python: impl Into<OsString>, repository_root: impl AsRef<Path>) -> Self {
        Self::new(python).args(["-m", "assistant"]).current_dir(repository_root)
    }

    pub fn packaged(executable: impl AsRef<Path>) -> Self {
        Self::new(executable.as_ref().as_os_str().to_owned())
    }

    pub fn packaged_executable_path(
        current_executable: impl AsRef<Path>,
        target_triple: &str,
    ) -> PathBuf {
        let filename =
            format!("oh-my-dream-assistant-{target_triple}{}", std::env::consts::EXE_SUFFIX);
        current_executable.as_ref().parent().unwrap_or_else(|| Path::new(".")).join(filename)
    }

    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args.extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));
        self
    }

    #[must_use]
    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    #[must_use]
    pub fn current_dir(mut self, directory: impl AsRef<Path>) -> Self {
        self.current_dir = Some(directory.as_ref().to_owned());
        self
    }

    pub(crate) fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args).envs(&self.env);
        if let Some(current_dir) = &self.current_dir {
            command.current_dir(current_dir);
        }
        command
    }
}

pub fn configured_assistant_command() -> Result<AssistantSidecarCommand, String> {
    if cfg!(debug_assertions) {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| assistant_error("resolve assistant repository", "missing parent"))?;
        return Ok(AssistantSidecarCommand::development(
            std::env::var_os("OH_MY_DREAM_PYTHON").unwrap_or_else(|| "python3".into()),
            repository_root,
        ));
    }
    let current_executable = std::env::current_exe()
        .map_err(|error| assistant_error("resolve assistant executable", error))?;
    let target = option_env!("OH_MY_DREAM_TARGET_TRIPLE")
        .ok_or_else(|| assistant_error("resolve assistant target", "target triple is missing"))?;
    Ok(AssistantSidecarCommand::packaged(AssistantSidecarCommand::packaged_executable_path(
        current_executable,
        target,
    )))
}

fn assistant_error(operation: &str, error: impl std::fmt::Display) -> String {
    tracing::error!(operation, error = %error, "assistant sidecar failed");
    format!("{operation}: {error}")
}
