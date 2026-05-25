mod api;
mod chat;
mod config;
mod tools;
mod web;

use clap::{Parser, Subcommand};
use colored::Colorize;

#[derive(Parser)]
#[command(name = "jellyfin-pulga", version, about = "Jellyfin media server toolkit")]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,

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
    Serve,
    /// List all Jellyfin users
    Users,
    /// List content reports
    Reports {
        /// Filter by status: open, reviewed, resolved, dismissed
        #[arg(long)]
        status: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let cfg = match config::Config::load(&cli.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} Failed to load config '{}': {e}", "Error:".red().bold(), cli.config);
            std::process::exit(1);
        }
    };

    match cli.command {
        Commands::Scan { include_subs } => cmd_scan(&cfg, include_subs),
        Commands::Clean { skip_subs } => cmd_clean(&cfg, skip_subs),
        Commands::Check { path, tag, output } => cmd_check(&cfg, path, tag, output).await,
        Commands::Flagged => cmd_flagged(&cfg).await,
        Commands::Review { item_id } => cmd_review(&cfg, &item_id).await,
        Commands::Export { output } => cmd_export(&cfg, output).await,
        Commands::Serve => cmd_serve(cfg).await,
        Commands::Users => cmd_users(&cfg).await,
        Commands::Reports { status } => cmd_reports(&cfg, status).await,
    }
}

fn cmd_scan(cfg: &config::Config, include_subs: bool) {
    println!("{}", "Scanning for junk files...".cyan().bold());
    let junk = tools::junk_cleaner::scan_junk(&cfg.media.paths, include_subs);
    tools::junk_cleaner::print_scan_results(&junk);
    if !junk.is_empty() {
        println!(
            "\nRun {} to delete these files.",
            "jellyfin-pulga clean".yellow()
        );
    }
}

fn cmd_clean(cfg: &config::Config, skip_subs: bool) {
    println!("{}", "Scanning for junk files...".cyan().bold());
    let junk = tools::junk_cleaner::scan_junk(&cfg.media.paths, true);

    if junk.is_empty() {
        println!("{}", "No junk files found.".green());
        return;
    }

    tools::junk_cleaner::print_scan_results(&junk);
    println!("\n{}", "Deleting junk files...".red().bold());
    let (deleted, failed) = tools::junk_cleaner::delete_junk(&junk, skip_subs);
    println!(
        "\n{}: {deleted} deleted, {failed} failed",
        "Done".green().bold()
    );
}

async fn cmd_check(cfg: &config::Config, path: Option<String>, tag: bool, output: Option<String>) {
    let paths = match path {
        Some(p) => vec![p.into()],
        None => cfg.media.paths.clone(),
    };

    println!("{}", "Finding media files...".cyan().bold());
    let files = tools::media_checker::find_media_files(&paths);
    println!("Found {} media files to check.\n", files.len());

    let results = tools::media_checker::check_all_files(&cfg.media.ffprobe_path, &files);
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
            if let Some(item) = all_items.iter().find(|i| i.path.as_deref() == Some(&problem.path)) {
                if !item.tags.contains(&"needs-review".to_string()) {
                    match api.add_tag(&item.id, "needs-review").await {
                        Ok(()) => {
                            println!("  {} {} ({})", "Tagged".yellow(), item.display_name(), problem.status);
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
                println!("  {} {} [{}]", item.id.dimmed(), item.display_name().bold(), path);
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
        Ok(()) => println!("{} Removed 'needs-review' tag from {item_id}", "Done".green().bold()),
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

async fn cmd_serve(cfg: config::Config) {
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
