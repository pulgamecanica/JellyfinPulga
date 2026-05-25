use crate::executor::Executor;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

pub fn find_media_files(executor: &Executor, paths: &[PathBuf]) -> Vec<String> {
    let mut files = Vec::new();
    for base in paths {
        let path_str = base.display().to_string();
        match executor.list_files(&path_str, MEDIA_EXTENSIONS) {
            Ok(found) => files.extend(found),
            Err(e) => eprintln!("  {} scanning {}: {e}", "Warning:".yellow(), path_str),
        }
    }
    files.sort();
    files
}

pub fn check_file(executor: &Executor, ffprobe_path: &str, file: &str) -> MediaFileStatus {
    let result = match executor.run_ffprobe(ffprobe_path, file) {
        Ok(r) => r,
        Err(e) => {
            return MediaFileStatus {
                path: file.to_string(),
                status: CheckStatus::Unreadable,
                details: format!("ffprobe execution failed: {e}"),
                duration_secs: None,
                codec: None,
                resolution: None,
            };
        }
    };

    if !result.success {
        return MediaFileStatus {
            path: file.to_string(),
            status: CheckStatus::Corrupted,
            details: format!("ffprobe error: {}", result.stderr.lines().next().unwrap_or("").trim()),
            duration_secs: None,
            codec: None,
            resolution: None,
        };
    }

    let probe: FfprobeOutput = match serde_json::from_str(&result.stdout) {
        Ok(p) => p,
        Err(e) => {
            return MediaFileStatus {
                path: file.to_string(),
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
    let resolution = video_stream.and_then(|s| match (s.width, s.height) {
        (Some(w), Some(h)) => Some(format!("{w}x{h}")),
        _ => None,
    });

    let duration_secs = probe
        .format
        .as_ref()
        .and_then(|f| f.duration.as_ref())
        .and_then(|d| d.parse::<f64>().ok());

    if !has_video {
        return MediaFileStatus {
            path: file.to_string(),
            status: CheckStatus::MissingVideo,
            details: "no video stream found".into(),
            duration_secs,
            codec,
            resolution,
        };
    }

    if !has_audio {
        return MediaFileStatus {
            path: file.to_string(),
            status: CheckStatus::MissingAudio,
            details: "no audio stream found (video-only file)".into(),
            duration_secs,
            codec,
            resolution,
        };
    }

    if !result.stderr.is_empty() {
        return MediaFileStatus {
            path: file.to_string(),
            status: CheckStatus::Corrupted,
            details: format!(
                "ffprobe warnings: {}",
                result.stderr.lines().next().unwrap_or("").trim()
            ),
            duration_secs,
            codec,
            resolution,
        };
    }

    MediaFileStatus {
        path: file.to_string(),
        status: CheckStatus::Ok,
        details: "file is valid".into(),
        duration_secs,
        codec,
        resolution,
    }
}

pub fn check_all_files(executor: &Executor, ffprobe_path: &str, files: &[String]) -> Vec<MediaFileStatus> {
    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
            .unwrap()
            .progress_chars("##-"),
    );

    let results: Vec<MediaFileStatus> = files
        .iter()
        .map(|file| {
            let filename = file.rsplit('/').next().unwrap_or("?");
            pb.set_message(filename.to_string());
            let result = check_file(executor, ffprobe_path, file);
            pb.inc(1);
            result
        })
        .collect();

    pb.finish_with_message("done");
    results
}

pub fn print_check_results(results: &[MediaFileStatus]) {
    let ok = results
        .iter()
        .filter(|r| r.status == CheckStatus::Ok)
        .count();
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
            CheckStatus::Unreadable | CheckStatus::ZeroLength => {
                result.status.to_string().red().to_string()
            }
            CheckStatus::MissingAudio => result.status.to_string().yellow().to_string(),
            CheckStatus::MissingVideo => result.status.to_string().yellow().bold().to_string(),
            CheckStatus::Ok => result.status.to_string().green().to_string(),
        };
        println!("  [{}] {}", status_str, result.path);
        println!("    {}", result.details.dimmed());
    }
}
