use std::process::{Command, Stdio};
use std::time::Duration;

pub(crate) fn run_output(
    command: &str,
    args: &[&str],
    _timeout: Duration,
) -> Result<String, String> {
    let output = Command::new(command)
        .args(args)
        .stderr(Stdio::null())
        .output()
        .map_err(|err| err.to_string())?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
