use crate::executor::Executor;
use colored::Colorize;
use std::path::PathBuf;

const JELLYFIN_SKIP_PATTERNS: &[&str] = &[
    ".trickplay/",
    "/metadata/",
    "/extrafanart/",
    "/extrathumbs/",
];

const JUNK_EXTENSIONS: &[&str] = &[
    "txt", "nfo", "url", "html", "exe", "torrent", "md5", "sha1",
];

const JUNK_FILENAMES: &[&str] = &[
    "Thumbs.db",
    ".DS_Store",
    "desktop.ini",
];

const JUNK_PATTERNS: &[&str] = &[
    "WWW.YTS",
    "www.YTS",
    "RARBG",
    "YIFYStatus",
    "YTSProxies",
    "YTSYifyUP",
    "VPPV.LA",
    "UIndex.org",
    "AhaShare",
    "demonoid",
    "SUJAIDR",
    "HDRush",
    "IranianTorrents",
];

const MEDIA_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ts",
];

const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "sub", "idx", "ass", "ssa", "vtt"];

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png"];

#[derive(Debug, Clone)]
pub struct JunkFile {
    pub path: String,
    pub reason: String,
    pub size: u64,
    pub category: JunkCategory,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JunkCategory {
    TorrentAd,
    MetadataJunk,
    PromoImage,
    Subtitle,
    SystemFile,
    #[allow(dead_code)]
    Other,
}

impl std::fmt::Display for JunkCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TorrentAd => write!(f, "torrent-ad"),
            Self::MetadataJunk => write!(f, "metadata"),
            Self::PromoImage => write!(f, "promo-image"),
            Self::Subtitle => write!(f, "subtitle"),
            Self::SystemFile => write!(f, "system-file"),
            Self::Other => write!(f, "other"),
        }
    }
}

pub fn scan_junk(executor: &Executor, paths: &[PathBuf], include_subtitles: bool) -> Vec<JunkFile> {
    let mut junk = Vec::new();

    for base_path in paths {
        let path_str = base_path.display().to_string();

        let files = match executor.list_all_files(&path_str) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  {} scanning {}: {e}", "Warning:".yellow(), path_str);
                continue;
            }
        };

        for (file_path, size) in files {
            if JELLYFIN_SKIP_PATTERNS.iter().any(|p| file_path.contains(p)) {
                continue;
            }

            let filename = file_path
                .rsplit('/')
                .next()
                .unwrap_or(&file_path);
            let ext = filename
                .rsplit('.')
                .next()
                .unwrap_or("")
                .to_lowercase();

            if MEDIA_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }

            if let Some(junk_file) = classify_junk(&file_path, filename, &ext, size, include_subtitles) {
                junk.push(junk_file);
            }
        }
    }

    junk.sort_by(|a, b| a.path.cmp(&b.path));
    junk
}

fn classify_junk(path: &str, filename: &str, ext: &str, size: u64, include_subtitles: bool) -> Option<JunkFile> {
    if JUNK_FILENAMES.contains(&filename) {
        return Some(JunkFile {
            path: path.to_string(),
            reason: "known junk filename".into(),
            size,
            category: JunkCategory::SystemFile,
        });
    }

    for pattern in JUNK_PATTERNS {
        if filename.contains(pattern) {
            return Some(JunkFile {
                path: path.to_string(),
                reason: format!("torrent site marker: {pattern}"),
                size,
                category: JunkCategory::TorrentAd,
            });
        }
    }

    if SUBTITLE_EXTENSIONS.contains(&ext) {
        if include_subtitles {
            return Some(JunkFile {
                path: path.to_string(),
                reason: "subtitle file (flagged for review)".into(),
                size,
                category: JunkCategory::Subtitle,
            });
        }
        return None;
    }

    if IMAGE_EXTENSIONS.contains(&ext) {
        return Some(JunkFile {
            path: path.to_string(),
            reason: "non-media image file".into(),
            size,
            category: JunkCategory::PromoImage,
        });
    }

    if JUNK_EXTENSIONS.contains(&ext) {
        return Some(JunkFile {
            path: path.to_string(),
            reason: format!("junk extension: .{ext}"),
            size,
            category: JunkCategory::MetadataJunk,
        });
    }

    None
}

pub fn print_scan_results(junk: &[JunkFile]) {
    if junk.is_empty() {
        println!("{}", "No junk files found.".green());
        return;
    }

    let total_size: u64 = junk.iter().map(|j| j.size).sum();
    let mut by_category: std::collections::HashMap<String, Vec<&JunkFile>> =
        std::collections::HashMap::new();

    for item in junk {
        by_category
            .entry(item.category.to_string())
            .or_default()
            .push(item);
    }

    println!(
        "\n{} {} junk files ({}):\n",
        "Found".yellow().bold(),
        junk.len(),
        format_bytes(total_size)
    );

    for (category, items) in &by_category {
        let cat_size: u64 = items.iter().map(|j| j.size).sum();
        println!(
            "  {} [{} files, {}]",
            category.cyan().bold(),
            items.len(),
            format_bytes(cat_size)
        );
        for item in items {
            println!(
                "    {} {} ({})",
                "•".dimmed(),
                item.path,
                item.reason.dimmed()
            );
        }
        println!();
    }
}

pub fn delete_junk(executor: &Executor, junk: &[JunkFile], skip_subtitles: bool) -> (usize, usize) {
    let mut deleted = 0;
    let mut failed = 0;

    for item in junk {
        if skip_subtitles && item.category == JunkCategory::Subtitle {
            continue;
        }
        match executor.delete_file(&item.path) {
            Ok(()) => {
                println!("  {} {}", "Deleted".red(), item.path);
                deleted += 1;
            }
            Err(e) => {
                eprintln!("  {} {} — {e}", "Failed".red().bold(), item.path);
                failed += 1;
            }
        }
    }

    (deleted, failed)
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
