use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: i64,
    pub room_id: String,
    pub user_id: String,
    pub username: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateMessage {
    pub id: i64,
    pub from_user_id: String,
    pub from_username: String,
    pub to_user_id: String,
    pub to_username: String,
    pub content: String,
    pub read: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentReport {
    pub id: i64,
    pub item_id: String,
    pub item_name: String,
    pub reporter_id: String,
    pub reporter_name: String,
    pub reason: ReportReason,
    pub details: String,
    pub status: ReportStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReportReason {
    Corrupted,
    WrongContent,
    MissingSubtitles,
    BadQuality,
    Duplicate,
    Other,
}

impl ReportReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Corrupted => "corrupted",
            Self::WrongContent => "wrong-content",
            Self::MissingSubtitles => "missing-subtitles",
            Self::BadQuality => "bad-quality",
            Self::Duplicate => "duplicate",
            Self::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "corrupted" => Self::Corrupted,
            "wrong-content" => Self::WrongContent,
            "missing-subtitles" => Self::MissingSubtitles,
            "bad-quality" => Self::BadQuality,
            "duplicate" => Self::Duplicate,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReportStatus {
    Open,
    Reviewed,
    Resolved,
    Dismissed,
}

impl ReportStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Reviewed => "reviewed",
            Self::Resolved => "resolved",
            Self::Dismissed => "dismissed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "open" => Self::Open,
            "reviewed" => Self::Reviewed,
            "resolved" => Self::Resolved,
            "dismissed" => Self::Dismissed,
            _ => Self::Open,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedUser {
    pub user_id: String,
    pub blocked_user_id: String,
    pub created_at: DateTime<Utc>,
}
