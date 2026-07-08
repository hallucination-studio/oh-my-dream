use crate::dto::AssistantSessionDto;
use crate::state::AppState;
use std::io::BufRead;
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use tracing::error;

/// Creates an assistant session, spawning the sidecar when app state enables it.
pub fn create_assistant_session(
    state: &AppState,
) -> Result<(AssistantSessionDto, Option<Child>), String> {
    let token = generate_token()?;
    let (port, process) = if state.assistant_sidecar_enabled {
        spawn_assistant_sidecar(state, &token)?
    } else {
        (reserve_loopback_port()?, None)
    };
    Ok((AssistantSessionDto { port, token }, process))
}

fn reserve_loopback_port() -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|source| assistant_error("reserve assistant port", source))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|source| assistant_error("read assistant port", source))
}

fn spawn_assistant_sidecar(state: &AppState, token: &str) -> Result<(u16, Option<Child>), String> {
    let mut child = Command::new("python3")
        .args(["-m", "assistant"])
        .env("OH_MY_DREAM_ASSISTANT_TOKEN", token)
        .env("OH_MY_DREAM_CONFIG_ROOT", &state.config_root)
        .env("OH_MY_DREAM_ALLOWED_ORIGINS", "tauri://localhost,http://localhost:5273")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|source| assistant_error("spawn assistant sidecar", source))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| assistant_error("read assistant sidecar stdout", "stdout was not piped"))?;
    let mut line = String::new();
    std::io::BufReader::new(stdout)
        .read_line(&mut line)
        .map_err(|source| assistant_error("read assistant sidecar port", source))?;
    let port = line
        .strip_prefix("PORT=")
        .and_then(|value| value.trim().parse::<u16>().ok())
        .ok_or_else(|| assistant_error("parse assistant sidecar port", line.trim()))?;
    Ok((port, Some(child)))
}

fn generate_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|source| assistant_error("generate assistant token", source))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn assistant_error(operation: &str, error: impl std::fmt::Display) -> String {
    error!(operation, error = %error, "assistant sidecar failed");
    format!("{operation}: {error}")
}
