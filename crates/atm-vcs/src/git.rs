//! Running git: the one place this crate starts it.

use std::path::Path;
use std::process::{Command, Output, Stdio};

use atm_common::py::strip;
use atm_common::{Error, Result};

/// Run git in `cwd`; none when it can't be started.
pub(super) fn run(args: &[&str], cwd: &Path) -> Option<Output> {
    run_fed(args, cwd, None)
}

/// Run git in `cwd`, with `input` on its stdin when given (paths for `--stdin`, never as arguments); none when it
/// can't be started or fed. The one place this crate starts git (I24).
pub(super) fn run_fed(args: &[&str], cwd: &Path, input: Option<&[u8]>) -> Option<Output> {
    use std::io::Write as _;
    let mut command = Command::new("git");
    command.args(args).current_dir(cwd);
    let Some(input) = input else {
        return command.stdin(Stdio::null()).output().ok();
    };
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    stdin.write_all(input).ok()?;
    drop(stdin);
    child.wait_with_output().ok()
}

/// A question for git: its output, stripped, or none when it fails.
pub(super) fn ask(args: &[&str], cwd: &Path) -> Option<String> {
    let output = run(args, cwd)?;
    output
        .status
        .success()
        .then(|| strip(&String::from_utf8_lossy(&output.stdout)).to_string())
}

/// A step that must work: git's output (stripped, unless `keep` is true), or an error with git's own message.
pub(super) fn step(args: &[&str], cwd: &Path, keep: bool) -> Result<String> {
    let output = run(args, cwd).ok_or_else(|| Error::failed("git isn't available"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let message = if strip(&stderr).is_empty() {
            stdout
        } else {
            stderr
        };
        let command = args
            .iter()
            .find(|a| !a.starts_with('-'))
            .copied()
            .unwrap_or_default();
        return Err(Error::failed(format!(
            "git {command} failed: {}",
            strip(&message)
        )));
    }
    Ok(if keep {
        stdout
    } else {
        strip(&stdout).to_string()
    })
}
