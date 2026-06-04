use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use serde_json::Value;

use crate::adapters::ToolDef;
use crate::tools::{truncate_to_byte_limit, MAX_READ_BYTES, ToolHandler};

pub struct RunCommandTool;

impl ToolHandler for RunCommandTool {
    fn name(&self) -> &'static str {
        "run_command"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "run_command",
            description: "Execute a shell command and return its stdout, stderr, and exit code. \
                Non-zero exit codes are returned as data, not as errors. \
                stdout and stderr are each capped at 100 KiB.",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute."
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Working directory for the command. Relative paths are resolved from the project working directory. Defaults to the project working directory."
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "description": "Timeout in milliseconds. Defaults to 300000 (5 minutes)."
                    }
                },
                "required": ["command"]
            }),
        }
    }

    fn execute(&self, args: &Value, working_dir: &Path) -> Result<String> {
        let command = args["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("run_command: 'command' must be a string"))?;

        let cwd: PathBuf = match args.get("cwd").and_then(|v| v.as_str()) {
            Some(c) => {
                let p = Path::new(c);
                if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    working_dir.join(p)
                }
            }
            None => working_dir.to_path_buf(),
        };

        let timeout_ms = args
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(300_000);

        #[cfg(target_os = "windows")]
        let mut child = Command::new("cmd")
            .args(["/C", command])
            .current_dir(&cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        #[cfg(not(target_os = "windows"))]
        let mut child = Command::new("sh")
            .args(["-c", command])
            .current_dir(&cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        // Wait with timeout using a poll loop to avoid blocking the executor indefinitely.
        let timeout = Duration::from_millis(timeout_ms);
        let start = std::time::Instant::now();

        let output = loop {
            match child.try_wait()? {
                Some(_) => break child.wait_with_output()?,
                None => {
                    if start.elapsed() >= timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Ok(format!(
                            "exit_code: -1\n\nstdout:\n(timed out after {}ms)\n\nstderr:\n(timed out after {}ms)",
                            timeout_ms, timeout_ms
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        };

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout_raw = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr_raw = String::from_utf8_lossy(&output.stderr).into_owned();

        let stdout = truncate_to_byte_limit(stdout_raw, MAX_READ_BYTES);
        let stderr = truncate_to_byte_limit(stderr_raw, MAX_READ_BYTES);

        Ok(format!(
            "exit_code: {}\n\nstdout:\n{}\n\nstderr:\n{}",
            exit_code,
            if stdout.is_empty() { "(empty)" } else { &stdout },
            if stderr.is_empty() { "(empty)" } else { &stderr },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn wd() -> PathBuf {
        std::env::current_dir().unwrap()
    }

    #[test]
    fn run_command_exit_zero() {
        let tool = RunCommandTool;
        #[cfg(target_os = "windows")]
        let args = serde_json::json!({ "command": "echo hello" });
        #[cfg(not(target_os = "windows"))]
        let args = serde_json::json!({ "command": "echo hello" });
        let result = tool.execute(&args, &wd()).unwrap();
        assert!(result.contains("exit_code: 0"), "result: {}", result);
        assert!(result.contains("hello"), "result: {}", result);
    }

    #[test]
    fn run_command_nonzero_exit_is_not_rust_error() {
        let tool = RunCommandTool;
        #[cfg(target_os = "windows")]
        let args = serde_json::json!({ "command": "exit 1" });
        #[cfg(not(target_os = "windows"))]
        let args = serde_json::json!({ "command": "exit 1" });
        let result = tool.execute(&args, &wd()).unwrap();
        assert!(result.contains("exit_code: 1"), "result: {}", result);
    }

    #[test]
    fn run_command_stderr_captured() {
        let tool = RunCommandTool;
        #[cfg(target_os = "windows")]
        let args = serde_json::json!({ "command": "echo err 1>&2" });
        #[cfg(not(target_os = "windows"))]
        let args = serde_json::json!({ "command": "echo err >&2" });
        let result = tool.execute(&args, &wd()).unwrap();
        assert!(result.contains("exit_code: 0"), "result: {}", result);
        assert!(result.contains("err"), "result: {}", result);
    }

    #[test]
    fn run_command_missing_command_arg_is_error() {
        let tool = RunCommandTool;
        let args = serde_json::json!({});
        let err = tool.execute(&args, &wd()).unwrap_err();
        assert!(err.to_string().contains("command"), "err: {}", err);
    }
}
