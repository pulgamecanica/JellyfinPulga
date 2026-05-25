use crate::config::{Config, SshConfig};
use colored::Colorize;
use std::process::Command;

const DEPLOY_DIR: &str = "~/jellyfin-pulga";

pub struct Deployer {
    ssh_target: String,
    ssh_port: u16,
    ssh_key: Option<String>,
    verbose: bool,
}

impl Deployer {
    pub fn new(ssh: &SshConfig, verbose: bool) -> Self {
        Self {
            ssh_target: format!("{}@{}", ssh.user, ssh.host),
            ssh_port: ssh.port,
            ssh_key: ssh.key_path.as_ref().map(|p| p.display().to_string()),
            verbose,
        }
    }

    fn ssh_cmd(&self) -> Command {
        let mut cmd = Command::new("ssh");
        cmd.arg("-o").arg("BatchMode=yes")
            .arg("-o").arg("ConnectTimeout=10")
            .arg("-p").arg(self.ssh_port.to_string());
        if let Some(ref key) = self.ssh_key {
            cmd.arg("-i").arg(key);
        }
        cmd.arg(&self.ssh_target);
        cmd
    }

    fn scp_cmd(&self) -> Command {
        let mut cmd = Command::new("scp");
        cmd.arg("-o").arg("BatchMode=yes")
            .arg("-P").arg(self.ssh_port.to_string());
        if let Some(ref key) = self.ssh_key {
            cmd.arg("-i").arg(key);
        }
        cmd
    }

    fn run_ssh(&self, command: &str) -> Result<String, String> {
        if self.verbose {
            println!("  {} {}", "$".dimmed(), command.dimmed());
        }
        let output = self.ssh_cmd()
            .arg(command)
            .output()
            .map_err(|e| format!("SSH failed: {e}"))?;

        if output.status.code() == Some(255) {
            return Err(format!("SSH connection to {} failed", self.ssh_target));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() && self.verbose && !stderr.is_empty() {
            eprintln!("    {}", stderr.trim().dimmed());
        }

        Ok(stdout)
    }

    fn run_ssh_check(&self, command: &str) -> Result<String, String> {
        let stdout = self.run_ssh(command)?;
        Ok(stdout.trim().to_string())
    }

    fn scp_to(&self, local: &str, remote: &str) -> Result<(), String> {
        let dest = format!("{}:{remote}", self.ssh_target);
        if self.verbose {
            println!("  {} {local} -> {dest}", "scp".dimmed());
        }
        let output = self.scp_cmd()
            .arg("-r")
            .arg(local)
            .arg(&dest)
            .output()
            .map_err(|e| format!("SCP failed: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("SCP failed: {}", stderr.trim()));
        }
        Ok(())
    }

    pub fn deploy(&self, config: &Config) -> Result<(), String> {
        self.step_check_connection()?;
        self.step_install_docker()?;
        self.step_prepare_directory()?;
        self.step_sync_files(config)?;
        self.step_build_and_start()?;
        self.step_verify(config)?;
        Ok(())
    }

    pub fn status(&self) -> Result<(), String> {
        self.step_check_connection()?;
        let out = self.run_ssh_check(&format!("cd {DEPLOY_DIR} && docker compose ps 2>/dev/null || echo 'not deployed'"))?;
        println!("{out}");
        Ok(())
    }

    pub fn logs(&self, lines: u32) -> Result<(), String> {
        self.step_check_connection()?;
        let out = self.run_ssh_check(&format!("cd {DEPLOY_DIR} && docker compose logs --tail {lines} 2>&1"))?;
        println!("{out}");
        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        self.step_check_connection()?;
        println!("{}", "Stopping container...".yellow());
        self.run_ssh(&format!("cd {DEPLOY_DIR} && docker compose down 2>&1"))?;
        println!("{}", "Stopped.".green());
        Ok(())
    }

    pub fn restart(&self) -> Result<(), String> {
        self.step_check_connection()?;
        println!("{}", "Restarting container...".yellow());
        self.run_ssh(&format!("cd {DEPLOY_DIR} && docker compose restart 2>&1"))?;
        println!("{}", "Restarted.".green());
        Ok(())
    }

    fn step_check_connection(&self) -> Result<(), String> {
        print!("{}", "Checking SSH connection... ".cyan());
        let result = self.run_ssh_check("echo ok")?;
        if result != "ok" {
            println!("{}", "FAILED".red());
            return Err(format!("SSH to {} failed", self.ssh_target));
        }
        println!("{} ({})", "OK".green(), self.ssh_target);
        Ok(())
    }

    fn step_install_docker(&self) -> Result<(), String> {
        print!("{}", "Checking Docker... ".cyan());
        let has_docker = self.run_ssh_check("command -v docker >/dev/null 2>&1 && echo yes || echo no")?;

        if has_docker == "yes" {
            let version = self.run_ssh_check("docker --version 2>/dev/null")?;
            println!("{} ({})", "installed".green(), version.trim());

            let has_compose = self.run_ssh_check("docker compose version >/dev/null 2>&1 && echo yes || echo no")?;
            if has_compose != "yes" {
                println!("\n  {} Docker Compose plugin missing. Install it:", "Warning:".yellow());
                println!("    {}", "sudo apt-get install -y docker-compose-plugin".yellow());
                return Err("Docker Compose not available".to_string());
            }
            return Ok(());
        }

        println!("{}", "not found".red());
        println!("\n  Docker is required on the server. Install it by running:\n");
        println!("    {}", format!("ssh {} -p {}", self.ssh_target, self.ssh_port).dimmed());
        println!("    {}", "curl -fsSL https://get.docker.com | sudo sh".yellow());
        println!("    {}", "sudo usermod -aG docker $USER".yellow());
        println!("    {}", "exit  # then re-run: jellyfin-pulga deploy up".dimmed());
        println!();
        return Err("Docker not installed — see instructions above".to_string())
    }

    fn step_prepare_directory(&self) -> Result<(), String> {
        print!("{}", "Preparing deploy directory... ".cyan());
        self.run_ssh(&format!("mkdir -p {DEPLOY_DIR}/static/css {DEPLOY_DIR}/static/js {DEPLOY_DIR}/src"))?;
        println!("{}", "OK".green());
        Ok(())
    }

    fn step_sync_files(&self, config: &Config) -> Result<(), String> {
        println!("{}", "Syncing files to server...".cyan());

        let project_dir = std::env::current_dir()
            .map_err(|e| format!("cannot get current dir: {e}"))?;
        let project = project_dir.display().to_string();

        self.scp_to(&format!("{project}/Dockerfile"), &format!("{DEPLOY_DIR}/Dockerfile"))?;
        self.scp_to(&format!("{project}/Cargo.toml"), &format!("{DEPLOY_DIR}/Cargo.toml"))?;
        self.scp_to(&format!("{project}/Cargo.lock"), &format!("{DEPLOY_DIR}/Cargo.lock"))?;
        self.scp_to(&format!("{project}/src"), &format!("{DEPLOY_DIR}/"))?;
        self.scp_to(&format!("{project}/static"), &format!("{DEPLOY_DIR}/"))?;

        self.generate_server_compose(config)?;
        self.generate_server_config(config)?;

        println!("  {}", "Files synced.".green());
        Ok(())
    }

    fn generate_server_compose(&self, config: &Config) -> Result<(), String> {
        let port = config.server.port;
        let media_volumes: Vec<String> = config
            .media
            .paths
            .iter()
            .map(|p| {
                let ps = p.display().to_string();
                format!("      - {ps}:{ps}:ro")
            })
            .collect();
        let volumes_str = media_volumes.join("\n");

        let compose = format!(
            r#"services:
  jellyfin-pulga:
    build: .
    container_name: jellyfin-pulga
    restart: unless-stopped
    ports:
      - "{port}:{port}"
    extra_hosts:
      - "host.docker.internal:host-gateway"
    volumes:
      - ./config.toml:/app/config.toml:ro
      - pulga-data:/app/data
{volumes_str}
    environment:
      - RUST_LOG=info
      - PULGA_DB_PATH=/app/data/jellyfin_pulga.db
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:{port}/api/users"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 30s

volumes:
  pulga-data:
"#
        );

        let escaped = compose.replace('\'', "'\\''");
        self.run_ssh(&format!(
            "cat > {DEPLOY_DIR}/docker-compose.yml << 'DEPLOYEOF'\n{escaped}\nDEPLOYEOF"
        ))?;
        Ok(())
    }

    fn generate_server_config(&self, config: &Config) -> Result<(), String> {
        let jf_url = if config.jellyfin.url.contains("localhost") || config.jellyfin.url.contains("127.0.0.1") {
            config.jellyfin.url.replace("localhost", "host.docker.internal")
                .replace("127.0.0.1", "host.docker.internal")
        } else {
            let ssh = config.execution.ssh.as_ref()
                .ok_or("SSH config required for deploy")?;
            config.jellyfin.url.replace(&ssh.host, "host.docker.internal")
        };

        let media_paths: Vec<String> = config
            .media
            .paths
            .iter()
            .map(|p| format!("\"{}\"", p.display()))
            .collect();
        let paths_str = media_paths.join(", ");

        let server_config = format!(
            r#"[jellyfin]
url = "{jf_url}"
api_key = "{api_key}"

[media]
paths = [{paths_str}]
ffprobe_path = "/usr/bin/ffprobe"

[server]
host = "0.0.0.0"
port = {port}

[execution]
mode = "local"
"#,
            api_key = config.jellyfin.api_key,
            port = config.server.port,
        );

        let escaped = server_config.replace('\'', "'\\''");
        self.run_ssh(&format!(
            "cat > {DEPLOY_DIR}/config.toml << 'DEPLOYEOF'\n{escaped}\nDEPLOYEOF"
        ))?;
        Ok(())
    }

    fn step_build_and_start(&self) -> Result<(), String> {
        println!("{}", "Building Docker image (this may take a few minutes)...".cyan());

        let docker_cmd = self.resolve_docker_cmd()?;

        let result = self.run_ssh(&format!(
            "cd {DEPLOY_DIR} && {docker_cmd} compose build --no-cache 2>&1"
        ))?;

        if self.verbose {
            for line in result.lines() {
                println!("  {}", line.dimmed());
            }
        }

        if result.contains("ERROR") && result.contains("failed") {
            return Err(format!("Docker build failed:\n{result}"));
        }

        println!("  {}", "Image built.".green());

        println!("{}", "Starting container...".cyan());
        let start_result = self.run_ssh(&format!(
            "cd {DEPLOY_DIR} && {docker_cmd} compose up -d 2>&1"
        ))?;

        if self.verbose {
            for line in start_result.lines() {
                println!("  {}", line.dimmed());
            }
        }

        println!("  {}", "Container started.".green());
        Ok(())
    }

    fn step_verify(&self, config: &Config) -> Result<(), String> {
        let ssh = config.execution.ssh.as_ref()
            .ok_or("SSH config required")?;
        let port = config.server.port;

        println!("{}", "Verifying deployment...".cyan());

        std::thread::sleep(std::time::Duration::from_secs(3));

        let docker_cmd = self.resolve_docker_cmd()?;
        let status = self.run_ssh_check(&format!(
            "cd {DEPLOY_DIR} && {docker_cmd} compose ps --format json 2>/dev/null | head -1"
        ))?;

        if status.is_empty() {
            return Err("Container does not appear to be running.".to_string());
        }

        let health = self.run_ssh_check(&format!(
            "curl -sf http://localhost:{port}/api/users >/dev/null 2>&1 && echo ok || echo fail"
        ))?;

        if health == "ok" {
            println!(
                "\n{} Deployed and healthy at {}:{}",
                "SUCCESS".green().bold(),
                ssh.host,
                port
            );
            println!(
                "  Web UI: http://{}:{}",
                ssh.host, port
            );
        } else {
            println!(
                "\n{} Container is running but health check failed (may still be starting).",
                "WARNING".yellow().bold()
            );
            println!(
                "  Try: jellyfin-pulga deploy status\n  Or:  jellyfin-pulga deploy logs"
            );
        }

        Ok(())
    }

    fn resolve_docker_cmd(&self) -> Result<String, String> {
        let direct = self.run_ssh_check("docker info >/dev/null 2>&1 && echo yes || echo no")?;
        if direct == "yes" {
            return Ok("docker".to_string());
        }

        let sudo = self.run_ssh_check("sudo docker info >/dev/null 2>&1 && echo yes || echo no")?;
        if sudo == "yes" {
            return Ok("sudo docker".to_string());
        }

        Err("Docker is not accessible. Try logging out and back in, or run: sudo usermod -aG docker $USER".to_string())
    }
}
