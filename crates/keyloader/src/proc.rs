use std::io::Write;
use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result};

/// Run a command feeding `input` on stdin, capturing stdout/stderr.
///
/// This is the only way secret material leaves this process: through a
/// pipe, never via argv (visible in `ps`) and never via the filesystem.
pub fn run_with_stdin(mut cmd: Command, input: &[u8]) -> Result<Output> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn `{program}`"))?;
    {
        let mut stdin = child.stdin.take().expect("stdin was piped");
        stdin
            .write_all(input)
            .with_context(|| format!("failed writing to `{program}` stdin"))?;
    }
    child
        .wait_with_output()
        .with_context(|| format!("failed waiting for `{program}`"))
}
