use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

const MEDIA_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ts",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFileStatus {
    pub path: String,
    pub status: CheckStatus,
    pub details: String,
    pub duration_secs: Option<f64>,
    pub codec: Option<String>,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CheckStatus {
    Ok,
    Corrupted,
    Unreadable,
    MissingAudio,
    MissingVideo,
    ZeroLength,
}

impl std::fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "ok"),
            Self::Corrupted => write!(f, "corrupted"),
            Self::Unreadable => write!(f, "unreadable"),
            Self::MissingAudio => write!(f, "missing-audio"),
            Self::MissingVideo => write!(f, "missing-video"),
            Self::ZeroLength => write!(f, "zero-length"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    streams: Vec<FfprobeStream>,
    #[serde(default)]
    format: Option<FfprobeFormat>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
}

pub fn find_media_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for base in paths {
        for entry in WalkDir::new(base).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if MEDIA_EXTENSIONS.contains(&ext.as_str()) {
                files.push(entry.into_path());
            }
        }
    }
    files.sort();
    files
}

pub fn check_file(ffprobe_path: &Path, file: &Path) -> MediaFileStatus {
    let path_str = file.display().to_string();

    if let Ok(meta) = file.metadata() {
        if meta.len() == 0 {
            return MediaFileStatus {
                path: path_str,
                status: CheckStatus::ZeroLength,
                details: "file is empty".into(),
                duration_secs: None,
                codec: None,
                resolution: None,
            };
        }
    }

    let output = Command::new(ffprobe_path)
        .args([
            "-v", "error",
            "-print_format", "json",
            "-show_streams",
            "-show_format",
        ])
        .arg(file)
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            return MediaFileStatus {
                path: path_str,
                status: CheckStatus::Unreadable,
                details: format!("ffprobe failed to execute: {e}"),
                duration_secs: None,
                codec: None,
                resolution: None,
            };
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return MediaFileStatus {
            path: path_str,
            status: CheckStatus::Corrupted,
            details: format!("ffprobe returned error: {}", stderr.trim()),
            duration_secs: None,
            codec: None,
            resolution: None,
        };
    }

    let probe: FfprobeOutput = match serde_json::from_slice(&output.stdout) {
        Ok(p) => p,
        Err(e) => {
            return MediaFileStatus {
                path: path_str,
                status: CheckStatus::Corrupted,
                details: format!("failed to parse ffprobe output: {e}"),
                duration_secs: None,
                codec: None,
                resolution: None,
            };
        }
    };

    let has_video = probe
        .streams
        .iter()
        .any(|s| s.codec_type.as_deref() == Some("video"));
    let has_audio = probe
        .streams
        .iter()
        .any(|s| s.codec_type.as_deref() == Some("audio"));

    let video_stream = probe
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video"));

    let codec = video_stream.and_then(|s| s.codec_name.clone());
    let resolution = video_stream.and_then(|s| {
        match (s.width, s.height) {
            (Some(w), Some(h)) => Some(format!("{w}x{h}")),
            _ => None,
        }
    });

    let duration_secs = probe
        .format
        .as_ref()
        .and_then(|f| f.duration.as_ref())
        .and_then(|d| d.parse::<f64>().ok());

    if !has_video {
        return MediaFileStatus {
            path: path_str,
            status: CheckStatus::MissingVideo,
            details: "no video stream found".into(),
            duration_secs,
            codec,
            resolution,
        };
    }

    if !has_audio {
        return MediaFileStatus {
            path: path_str,
            status: CheckStatus::MissingAudio,
            details: "no audio stream found (video-only file)".into(),
            duration_secs,
            codec,
            resolution,
        };
    }

    if !stderr.is_empty() {
        return MediaFileStatus {
            path: path_str,
            status: CheckStatus::Corrupted,
            details: format!("ffprobe warnings: {}", stderr.trim().lines().next().unwrap_or("")),
            duration_secs,
            codec,
            resolution,
        };
    }

    MediaFileStatus {
        path: path_str,
        status: CheckStatus::Ok,
        details: "file is valid".into(),
        duration_secs,
        codec,
        resolution,
    }
}

pub fn check_all_files(ffprobe_path: &Path, files: &[PathBuf]) -> Vec<MediaFileStatus> {
    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
            .unwrap()
            .progress_chars("█▓░"),
    );

    let results: Vec<MediaFileStatus> = files
        .iter()
        .map(|file| {
            pb.set_message(
                file.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string(),
            );
            let result = check_file(ffprobe_path, file);
            pb.inc(1);
            result
        })
        .collect();

    pb.finish_with_message("done");
    results
}

pub fn print_check_results(results: &[MediaFileStatus]) {
    let ok = results.iter().filter(|r| r.status == CheckStatus::Ok).count();
    let problems: Vec<&MediaFileStatus> = results
        .iter()
        .filter(|r| r.status != CheckStatus::Ok)
        .collect();

    println!(
        "\n{} {} files: {} ok, {} with issues\n",
        "Checked".green().bold(),
        results.len(),
        ok,
        problems.len()
    );

    if problems.is_empty() {
        println!("{}", "All files passed validation.".green());
        return;
    }

    for result in &problems {
        let status_str = match result.status {
            CheckStatus::Corrupted => result.status.to_string().red().bold().to_string(),
            CheckStatus::Unreadable => result.status.to_string().red().to_string(),
            CheckStatus::ZeroLength => result.status.to_string().red().to_string(),
            CheckStatus::MissingAudio => result.status.to_string().yellow().to_string(),
            CheckStatus::MissingVideo => result.status.to_string().yellow().bold().to_string(),
            CheckStatus::Ok => result.status.to_string().green().to_string(),
        };
        println!("  [{}] {}", status_str, result.path);
        println!("    {}", result.details.dimmed());
    }
}
