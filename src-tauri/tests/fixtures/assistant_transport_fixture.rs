use std::env;
use std::io::{self, Write};

const LARGE_FRAME_COUNT: u64 = 256;
const LARGE_PAYLOAD_SIZE: usize = 2_048;

fn main() -> io::Result<()> {
    let mode = env::args()
        .nth(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "fixture mode is required"))?;
    match mode.as_str() {
        "echo" => echo(),
        "wait" => wait_for_stdin_eof(),
        "large-output" => large_output_after_stdin_eof(),
        "invalid-output" => invalid_output_after_stdin_eof(),
        "invalid-then-large-output" => invalid_then_large_output_after_stdin_eof(),
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "unknown fixture mode")),
    }
}

fn echo() -> io::Result<()> {
    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    io::copy(&mut stdin, &mut stdout)?;
    stdout.flush()
}

fn wait_for_stdin_eof() -> io::Result<()> {
    io::copy(&mut io::stdin().lock(), &mut io::sink())?;
    Ok(())
}

fn large_output_after_stdin_eof() -> io::Result<()> {
    wait_for_stdin_eof()?;
    let mut stdout = io::stdout().lock();
    write_large_output(&mut stdout)
}

fn write_large_output(stdout: &mut impl Write) -> io::Result<()> {
    let payload = "x".repeat(LARGE_PAYLOAD_SIZE);
    for sequence in 0..LARGE_FRAME_COUNT {
        writeln!(
            stdout,
            "{{\"protocol_version\":1,\"sequence\":{sequence},\"kind\":\"assistant_token\",\"payload\":{{\"text\":\"{payload}\"}}}}"
        )?;
    }
    stdout.flush()
}

fn invalid_output_after_stdin_eof() -> io::Result<()> {
    wait_for_stdin_eof()?;
    io::stdout().lock().write_all(b"not-json\n")
}

fn invalid_then_large_output_after_stdin_eof() -> io::Result<()> {
    wait_for_stdin_eof()?;
    let mut stdout = io::stdout().lock();
    stdout.write_all(b"not-json\n")?;
    stdout.flush()?;
    write_large_output(&mut stdout)
}
