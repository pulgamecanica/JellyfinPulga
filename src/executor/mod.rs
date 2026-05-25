use crate::config::{ExecutionConfig, ExecutionMode};
use std::process::Command;

pub struct Executor {
    mode: ExecutionMode,
    ssh_target: Option<String>,
    ssh_port: u16,
    ssh_key: Option<String>,
}

#[derive(Debug)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

impl Executor {
    pub fn from_config(config: &ExecutionConfig) -> Self {
        let ssh_target = config.ssh.as_ref().map(|s| format!("{}@{}", s.user, s.host));
        let ssh_port = config.ssh.as_ref().map_or(22, |s| s.port);
        let ssh_key = config
            .ssh
            .as_ref()
            .and_then(|s| s.key_path.as_ref())
            .map(|p| p.display().to_string());

        Self {
            mode: config.mode.clone(),
            ssh_target,
            ssh_port,
            ssh_key,
        }
    }

    pub fn run_cmd(&self, command: &str) -> Result<ExecResult, String> {
        match &self.mode {
            ExecutionMode::Local => self.run_local(command),
            ExecutionMode::Ssh => self.run_ssh(command),
        }
    }

    fn run_local(&self, command: &str) -> Result<ExecResult, String> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|e| format!("failed to execute command: {e}"))?;

        Ok(ExecResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            success: output.status.success(),
        })
    }

    fn run_ssh(&self, command: &str) -> Result<ExecResult, String> {
        let target = self
            .ssh_target
            .as_ref()
            .ok_or("SSH config missing: no host/user defined in [execution.ssh]")?;

        let mut cmd = Command::new("ssh");
        cmd.arg("-o").arg("BatchMode=yes")
            .arg("-o").arg("ConnectTimeout=10")
            .arg("-p").arg(self.ssh_port.to_string());

        if let Some(ref key) = self.ssh_key {
            cmd.arg("-i").arg(key);
        }

        cmd.arg(target).arg(command);

        let output = cmd
            .output()
            .map_err(|e| format!("SSH execution failed: {e}"))?;

        if output.status.code() == Some(255) {
            return Err(format!("SSH connection to {target} failed — check host, user, and key"));
        }

        Ok(ExecResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            success: output.status.success(),
        })
    }

    pub fn list_files(&self, path: &str, extensions: &[&str]) -> Result<Vec<String>, String> {
        let ext_args: Vec<String> = extensions
            .iter()
            .enumerate()
            .flat_map(|(i, ext)| {
                let prefix = if i == 0 { "(" } else { "-o" };
                vec![prefix.to_string(), "-name".to_string(), format!("*.{ext}")]
            })
            .chain(std::iter::once(")".to_string()))
            .collect();

        let find_cmd = format!(
            "find {} -type f {}",
            shell_escape(path),
            ext_args.join(" ")
        );

        let result = self.run_cmd(&find_cmd)?;
        if !result.success && !result.stderr.trim().is_empty() {
            return Err(format!("find failed: {}", result.stderr.trim()));
        }

        Ok(result
            .stdout
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect())
    }

    pub fn list_all_files(&self, path: &str) -> Result<Vec<(String, u64)>, String> {
        let cmd = format!(
            "find {} -type f -printf '%s\\t%p\\n'",
            shell_escape(path)
        );

        let result = self.run_cmd(&cmd)?;
        if !result.success && !result.stderr.trim().is_empty() {
            return Err(format!("find failed: {}", result.stderr.trim()));
        }

        Ok(result
            .stdout
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|line| {
                let mut parts = line.splitn(2, '\t');
                let size: u64 = parts.next()?.parse().ok()?;
                let path = parts.next()?.to_string();
                Some((path, size))
            })
            .collect())
    }

    pub fn delete_file(&self, path: &str) -> Result<(), String> {
        let cmd = format!("rm -f {}", shell_escape(path));
        let result = self.run_cmd(&cmd)?;
        if !result.success {
            return Err(format!("rm failed: {}", result.stderr.trim()));
        }
        Ok(())
    }

    pub fn run_ffprobe(&self, ffprobe_path: &str, media_path: &str) -> Result<ExecResult, String> {
        let cmd = format!(
            "{} -v error -print_format json -show_streams -show_format {}",
            shell_escape(ffprobe_path),
            shell_escape(media_path)
        );
        self.run_cmd(&cmd)
    }

    pub fn test_connection(&self) -> Result<(), String> {
        match &self.mode {
            ExecutionMode::Local => Ok(()),
            ExecutionMode::Ssh => {
                let result = self.run_cmd("echo ok")?;
                if result.stdout.trim() == "ok" {
                    Ok(())
                } else {
                    Err("SSH connection test returned unexpected output".to_string())
                }
            }
        }
    }

    pub fn describe(&self) -> String {
        match &self.mode {
            ExecutionMode::Local => "local".to_string(),
            ExecutionMode::Ssh => self
                .ssh_target
                .as_ref()
                .map_or_else(|| "ssh (unconfigured)".to_string(), |t| format!("ssh ({t})")),
        }
    }
}

fn shell_escape(s: &str) -> String {
    if s.contains(|c: char| c.is_whitespace() || "\"'\\$`!#&|;(){}[]<>?*~".contains(c)) {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}
