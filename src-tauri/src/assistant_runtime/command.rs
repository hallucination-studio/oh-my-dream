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

    /// Creates the development command using the same framed stdio entrypoint.
    pub fn development(python: impl Into<OsString>, repository_root: impl AsRef<Path>) -> Self {
        Self::new(python).args(["-m", "assistant.protocol_v1_app"]).current_dir(repository_root)
    }

    /// Creates a command for a frozen executable shipped with the desktop app.
    pub fn packaged(executable: impl AsRef<Path>) -> Self {
        Self::new(executable.as_ref().as_os_str().to_owned())
    }

    /// Resolves the target-triple-suffixed executable beside the desktop binary.
    pub fn packaged_executable_path(
        current_executable: impl AsRef<Path>,
        target_triple: &str,
    ) -> PathBuf {
        let filename =
            format!("oh-my-dream-assistant-{target_triple}{}", std::env::consts::EXE_SUFFIX);
        current_executable.as_ref().parent().unwrap_or_else(|| Path::new(".")).join(filename)
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

    pub(crate) fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args).envs(&self.env);
        if let Some(current_dir) = &self.current_dir {
            command.current_dir(current_dir);
        }
        command
    }
}

#[cfg(test)]
mod tests {
    use super::AssistantSidecarCommand;
    use std::path::Path;

    #[test]
    fn packaged_path_uses_target_triple_next_to_desktop_binary() {
        let path = AssistantSidecarCommand::packaged_executable_path(
            "/Applications/oh-my-dream.app/Contents/MacOS/oh-my-dream",
            "aarch64-apple-darwin",
        );
        assert_eq!(
            path,
            Path::new(
                "/Applications/oh-my-dream.app/Contents/MacOS/oh-my-dream-assistant-aarch64-apple-darwin"
            )
        );
    }

    #[test]
    fn development_command_uses_stdio_module() {
        let command = AssistantSidecarCommand::development("python3", "/workspace");
        let debug = format!("{command:?}");
        assert!(debug.contains("assistant.protocol_v1_app"));
        assert!(debug.contains("/workspace"));
    }
}
