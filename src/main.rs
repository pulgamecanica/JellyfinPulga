mod api;
mod chat;
mod config;
mod deploy;
mod executor;
mod tools;
mod web;

use clap::{Parser, Subcommand};
use colored::Colorize;
use executor::Executor;

#[derive(Parser)]
#[command(name = "jellyfin-pulga", version, about = "Jellyfin media server toolkit")]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Print detailed information about what's happening
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan media directories for junk files (dry run)
    Scan {
        /// Include subtitle files in the scan
        #[arg(long)]
        include_subs: bool,
    },
    /// Delete detected junk files
    Clean {
        /// Skip subtitle files even if scanned
        #[arg(long)]
        skip_subs: bool,
    },
    /// Check media files for corruption using ffprobe
    Check {
        /// Only check files in this subdirectory
        #[arg(long)]
        path: Option<String>,
        /// Tag corrupted files in Jellyfin with 'needs-review'
        #[arg(long)]
        tag: bool,
        /// Save results to a JSON file
        #[arg(long)]
        output: Option<String>,
    },
    /// List items flagged with 'needs-review' in Jellyfin
    Flagged,
    /// Mark a flagged item as reviewed (removes the tag)
    Review {
        /// Jellyfin item ID to mark as reviewed
        item_id: String,
    },
    /// Export flagged items as a TSV list
    Export {
        /// Output file path (stdout if omitted)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Start the web server (chat, reports, management UI)
    Serve {
        /// Override the port from config
        #[arg(short, long)]
        port: Option<u16>,
        /// Override the host/bind address from config
        #[arg(long)]
        host: Option<String>,
    },
    /// List all Jellyfin users
    Users,
    /// List content reports
    Reports {
        /// Filter by status: open, reviewed, resolved, dismissed
        #[arg(long)]
        status: Option<String>,
    },
    /// Deploy to a remote server via SSH (installs Docker if needed)
    Deploy {
        #[command(subcommand)]
        action: DeployAction,
    },
}

#[derive(Subcommand)]
enum DeployAction {
    /// Full deploy: install Docker, build image, start container
    Up,
    /// Show container status
    Status,
    /// Show container logs
    Logs {
        /// Number of log lines to show
        #[arg(short = 'n', long, default_value = "50")]
        lines: u32,
    },
    /// Stop the deployed container
    Stop,
    /// Restart the deployed container
    Restart,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let verbose = cli.verbose;

    let cfg = match config::Config::load(&cli.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} Failed to load config '{}': {e}", "Error:".red().bold(), cli.config);
            std::process::exit(1);
        }
    };

    let exec = Executor::from_config(&cfg.execution);

    if verbose {
        println!("{}", "Config loaded:".dimmed());
        println!("  Jellyfin URL: {}", cfg.jellyfin.url.cyan());
        println!("  Media paths:  {:?}", cfg.media.paths);
        println!("  ffprobe:      {}", cfg.media.ffprobe_path.display());
        println!("  Server:       {}:{}", cfg.server.host, cfg.server.port);
        println!("  Execution:    {}\n", exec.describe().cyan());
    }

    let needs_exec = matches!(
        cli.command,
        Commands::Scan { .. } | Commands::Clean { .. } | Commands::Check { .. }
    );
    if needs_exec {
        if let Err(e) = exec.test_connection() {
            eprintln!("{} Execution backend unavailable: {e}", "Error:".red().bold());
            std::process::exit(1);
        }
        if verbose {
            println!("{} Execution backend ({}) is reachable.\n", "OK:".green(), exec.describe());
        }
    }

    let needs_api = !matches!(
        cli.command,
        Commands::Scan { .. } | Commands::Clean { .. } | Commands::Reports { .. } | Commands::Deploy { .. }
    );
    if needs_api {
        let api = api::JellyfinApi::new(&cfg.jellyfin);
        match api.health_check().await {
            Ok(info) => {
                if verbose {
                    println!(
                        "{} {} v{} ({})\n",
                        "OK:".green(),
                        info.server_name,
                        info.version.cyan(),
                        cfg.jellyfin.url
                    );
                }
            }
            Err(e) => {
                eprintln!("{} {e}", "Error:".red().bold());
                eprintln!(
                    "  Check that Jellyfin is running at {} and your API key is valid.",
                    cfg.jellyfin.url.yellow()
                );
                std::process::exit(1);
            }
        }
    }

    match cli.command {
        Commands::Scan { include_subs } => cmd_scan(&cfg, &exec, include_subs, verbose),
        Commands::Clean { skip_subs } => cmd_clean(&cfg, &exec, skip_subs, verbose),
        Commands::Check { path, tag, output } => cmd_check(&cfg, &exec, path, tag, output, verbose).await,
        Commands::Flagged => cmd_flagged(&cfg).await,
        Commands::Review { item_id } => cmd_review(&cfg, &item_id).await,
        Commands::Export { output } => cmd_export(&cfg, output).await,
        Commands::Serve { port, host } => cmd_serve(cfg, port, host).await,
        Commands::Users => cmd_users(&cfg).await,
        Commands::Reports { status } => cmd_reports(&cfg, status).await,
        Commands::Deploy { action } => cmd_deploy(&cfg, action, verbose),
    }
}

fn cmd_deploy(cfg: &config::Config, action: DeployAction, verbose: bool) {
    let ssh = match &cfg.execution.ssh {
        Some(s) => s,
        None => {
            eprintln!(
                "{} Deploy requires [execution.ssh] in config.toml",
                "Error:".red().bold()
            );
            std::process::exit(1);
        }
    };

    let deployer = deploy::Deployer::new(ssh, verbose);

    let result = match action {
        DeployAction::Up => deployer.deploy(cfg),
        DeployAction::Status => deployer.status(),
        DeployAction::Logs { lines } => deployer.logs(lines),
        DeployAction::Stop => deployer.stop(),
        DeployAction::Restart => deployer.restart(),
    };

    if let Err(e) = result {
        eprintln!("{} {e}", "Error:".red().bold());
        std::process::exit(1);
    }
}

fn cmd_scan(cfg: &config::Config, exec: &Executor, include_subs: bool, verbose: bool) {
    println!("{}", "Scanning for junk files...".cyan().bold());
    if verbose {
        println!("  Paths: {:?}", cfg.media.paths);
        println!("  Include subtitles: {include_subs}");
    }
    let junk = tools::junk_cleaner::scan_junk(exec, &cfg.media.paths, include_subs);
    tools::junk_cleaner::print_scan_results(&junk);
    if !junk.is_empty() {
        println!(
            "\nRun {} to delete these files.",
            "jellyfin-pulga clean".yellow()
        );
    }
}

fn cmd_clean(cfg: &config::Config, exec: &Executor, skip_subs: bool, verbose: bool) {
    println!("{}", "Scanning for junk files...".cyan().bold());
    if verbose {
        println!("  Skip subtitles: {skip_subs}");
    }
    let junk = tools::junk_cleaner::scan_junk(exec, &cfg.media.paths, true);

    if junk.is_empty() {
        println!("{}", "No junk files found.".green());
        return;
    }

    tools::junk_cleaner::print_scan_results(&junk);
    println!("\n{}", "Deleting junk files...".red().bold());
    let (deleted, failed) = tools::junk_cleaner::delete_junk(exec, &junk, skip_subs);
    println!(
        "\n{}: {deleted} deleted, {failed} failed",
        "Done".green().bold()
    );
}

async fn cmd_check(
    cfg: &config::Config,
    exec: &Executor,
    path: Option<String>,
    tag: bool,
    output: Option<String>,
    verbose: bool,
) {
    let paths = match path {
        Some(p) => vec![p.into()],
        None => cfg.media.paths.clone(),
    };

    let ffprobe = cfg.media.ffprobe_path.display().to_string();

    println!("{}", "Finding media files...".cyan().bold());
    if verbose {
        println!("  Paths: {:?}", paths);
        println!("  ffprobe: {ffprobe}");
        println!("  Tag corrupted: {tag}");
    }
    let files = tools::media_checker::find_media_files(exec, &paths);
    println!("Found {} media files to check.\n", files.len());

    let results = tools::media_checker::check_all_files(exec, &ffprobe, &files);
    tools::media_checker::print_check_results(&results);

    if let Some(out_path) = output {
        let json = serde_json::to_string_pretty(&results).unwrap();
        if let Err(e) = std::fs::write(&out_path, json) {
            eprintln!("{} Failed to write {out_path}: {e}", "Error:".red().bold());
        } else {
            println!("\nResults saved to {out_path}");
        }
    }

    if tag {
        let api = api::JellyfinApi::new(&cfg.jellyfin);
        let problems: Vec<_> = results
            .iter()
            .filter(|r| r.status != tools::media_checker::CheckStatus::Ok)
            .collect();

        if problems.is_empty() {
            println!("{}", "\nNo items to tag.".green());
            return;
        }

        println!("\n{}", "Tagging corrupted items in Jellyfin...".cyan());
        let all_items = match api.get_all_items("Movie,Episode", "Path,Tags").await {
            Ok(items) => items,
            Err(e) => {
                eprintln!("{} Failed to fetch items: {e}", "Error:".red().bold());
                return;
            }
        };

        let mut tagged = 0;
        for problem in &problems {
            if let Some(item) = all_items
                .iter()
                .find(|i| i.path.as_deref() == Some(&problem.path))
            {
                if !item.tags.contains(&"needs-review".to_string()) {
                    match api.add_tag(&item.id, "needs-review").await {
                        Ok(()) => {
                            println!(
                                "  {} {} ({})",
                                "Tagged".yellow(),
                                item.display_name(),
                                problem.status
                            );
                            tagged += 1;
                        }
                        Err(e) => eprintln!("  {} {} — {e}", "Failed".red(), item.display_name()),
                    }
                }
            }
        }
        println!("\n{tagged} items tagged with 'needs-review'");
    }
}

async fn cmd_flagged(cfg: &config::Config) {
    let api = api::JellyfinApi::new(&cfg.jellyfin);
    match api.get_items_by_tag("needs-review", "Movie,Episode").await {
        Ok(items) => {
            if items.is_empty() {
                println!("{}", "No flagged items.".green());
                return;
            }
            println!(
                "\n{} flagged items:\n",
                items.len().to_string().yellow().bold()
            );
            for item in &items {
                let path = item.path.as_deref().unwrap_or("?");
                println!(
                    "  {} {} [{}]",
                    item.id.dimmed(),
                    item.display_name().bold(),
                    path
                );
            }
            println!(
                "\nUse {} to remove the flag.",
                "jellyfin-pulga review <item-id>".yellow()
            );
        }
        Err(e) => eprintln!("{} {e}", "Error:".red().bold()),
    }
}

async fn cmd_review(cfg: &config::Config, item_id: &str) {
    let api = api::JellyfinApi::new(&cfg.jellyfin);
    match api.remove_tag(item_id, "needs-review").await {
        Ok(()) => println!(
            "{} Removed 'needs-review' tag from {item_id}",
            "Done".green().bold()
        ),
        Err(e) => eprintln!("{} {e}", "Error:".red().bold()),
    }
}

async fn cmd_export(cfg: &config::Config, output: Option<String>) {
    let api = api::JellyfinApi::new(&cfg.jellyfin);
    match api.get_items_by_tag("needs-review", "Movie,Episode").await {
        Ok(items) => {
            let mut lines = vec!["Name\tType\tPath".to_string()];
            for item in &items {
                let path = item.path.as_deref().unwrap_or("");
                lines.push(format!("{}\t{}\t{}", item.display_name(), item.r#type, path));
            }
            let content = lines.join("\n");

            match output {
                Some(path) => {
                    if let Err(e) = std::fs::write(&path, &content) {
                        eprintln!("{} {e}", "Error:".red().bold());
                    } else {
                        println!("Exported {} items to {path}", items.len());
                    }
                }
                None => println!("{content}"),
            }
        }
        Err(e) => eprintln!("{} {e}", "Error:".red().bold()),
    }
}

async fn cmd_serve(mut cfg: config::Config, port: Option<u16>, host: Option<String>) {
    if let Some(p) = port {
        cfg.server.port = p;
    }
    if let Some(h) = host {
        cfg.server.host = h;
    }
    if let Err(e) = web::start_server(cfg).await {
        eprintln!("{} {e}", "Error:".red().bold());
        std::process::exit(1);
    }
}

async fn cmd_users(cfg: &config::Config) {
    let api = api::JellyfinApi::new(&cfg.jellyfin);
    match api.get_users().await {
        Ok(users) => {
            println!("\n{} Jellyfin users:\n", users.len());
            for user in &users {
                println!("  {} {}", user.id.dimmed(), user.name.bold());
            }
        }
        Err(e) => eprintln!("{} {e}", "Error:".red().bold()),
    }
}

async fn cmd_reports(_cfg: &config::Config, status: Option<String>) {
    let db = match chat::db::ChatDb::new("jellyfin_pulga.db") {
        Ok(db) => db,
        Err(e) => {
            eprintln!("{} {e}", "Error:".red().bold());
            return;
        }
    };

    match db.get_reports(status.as_deref()) {
        Ok(reports) => {
            if reports.is_empty() {
                println!("{}", "No reports found.".green());
                return;
            }
            println!("\n{} reports:\n", reports.len());
            for r in &reports {
                println!(
                    "  #{} [{}] {} — {} (by {})",
                    r.id,
                    r.status.as_str().yellow(),
                    r.item_name.bold(),
                    r.reason.as_str(),
                    r.reporter_name
                );
                if !r.details.is_empty() {
                    println!("    {}", r.details.dimmed());
                }
            }
        }
        Err(e) => eprintln!("{} {e}", "Error:".red().bold()),
    }
}
